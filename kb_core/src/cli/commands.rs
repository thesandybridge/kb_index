use crate::chroma::{self, SearchResult};
use crate::config;
use crate::embedding;
use crate::llm;
use crate::utils;
use crate::state::{IndexState, IndexedChunk};
use futures::stream::{FuturesUnordered, StreamExt};
use std::time::{Duration, UNIX_EPOCH};
use tokio::time::sleep;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use std::fs;
use std::path::Path;
use uuid::Uuid;

const BATCH_SIZE: usize = 8;

pub fn handle_config(set_api_key: Option<String>, show: bool) -> anyhow::Result<()> {
    let config_path = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Unable to determine config directory"))?
        .join("kb-index")
        .join("config.toml");

    // Load existing config
    let mut config = config::load_config()?;

    // Set API key if provided
    if let Some(api_key) = set_api_key {
        config.openai_api_key = Some(api_key);

        // Save updated config
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(&config)?;
        fs::write(&config_path, content)?;

        println!("✅ API key saved to config file: {}", config_path.display());
    }

    // Show config if requested
    if show {
        println!("Configuration file: {}", config_path.display());
        println!("Chroma host: {}", config.chroma_host);
        println!("OpenAI API key: {}", match config.openai_api_key {
            Some(key) if !key.is_empty() => "Set (hidden for security)",
            _ => "Not set"
        });
        println!("OpenAI Completion Model: {}", config.openai_completion_model);
        println!("OpenAI Embedding Model: {}", config.openai_embedding_model);
        println!("Supported Extensions: {:?}", config.file_extensions.unwrap());
        println!("Syntax Theme: {:?}", config.syntax_theme.unwrap());
        // Check environment variable
        match std::env::var("OPENAI_API_KEY") {
            Ok(_) => println!("OPENAI_API_KEY environment variable: Set (hidden for security)"),
            Err(_) => println!("OPENAI_API_KEY environment variable: Not set"),
        }
    }

    Ok(())
}

pub async fn handle_index(client: &Client, path: &Path) -> anyhow::Result<()> {
    let paths = utils::collect_files(path)?;
    let total_files = paths.len() as u64;
    let pb = ProgressBar::new(total_files);
    pb.set_style(
        ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] {msg}")
            .unwrap()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ "),
    );

    let config_dir = config::get_config_dir()?;
    let mut state = IndexState::load(&config_dir)?;

    for path in paths {
        pb.set_message(format!("Indexing {}", path.display()));
        let metadata = fs::metadata(&path)?;
        let modified = metadata.modified()?.duration_since(UNIX_EPOCH)?.as_secs();
        let file_str = path.to_string_lossy().to_string();

        // Skip if file unchanged
        if let Some(prev) = state.get_last_modified(&file_str) {
            if prev == modified {
                pb.inc(1);
                continue;
            }
        }

        let content = fs::read_to_string(&path)?;
        let chunks = utils::chunk_text(&content);
        let prev_chunks = state.get_file_chunks(&file_str).cloned().unwrap_or_default();
        let mut new_chunks = Vec::new();
        let mut chunk_info = Vec::new();

        for chunk in &chunks {
            if chunk.trim().is_empty() || chunk.len() > 100_000 {
                continue;
            }

            let hash = IndexState::hash_chunk(chunk);
            if IndexState::has_chunk(&prev_chunks, &hash) {
                continue;
            }

            chunk_info.push((chunk.clone(), hash));
        }

        for batch in chunk_info.chunks(BATCH_SIZE) {
            let mut tasks = FuturesUnordered::new();

            for (chunk, hash) in batch.iter().cloned() {
                let client = client.clone();
                let path = path.to_path_buf();
                let pb = pb.clone();
                tasks.push(async move {
                    sleep(Duration::from_millis(100)).await;
                    let embedding = embedding::get_embedding(&client, &chunk).await?;
                    let id = Uuid::new_v4().to_string();
                    chroma::send_to_chroma(&client, &id, &chunk, &embedding, &path, &pb).await?;
                    Ok::<_, anyhow::Error>(IndexedChunk { id, hash })
                });
            }

            while let Some(result) = tasks.next().await {
                if let Ok(chunk) = result {
                    new_chunks.push(chunk);
                }
            }
        }

        if !new_chunks.is_empty() {
            let mut updated_chunks = prev_chunks.clone();
            let mut removed_chunks = Vec::new();

            updated_chunks.retain(|c| {
                let keep = new_chunks.iter().all(|n| n.hash != c.hash);
                if !keep {
                    removed_chunks.push(c.clone());
                }
                keep
            });

            updated_chunks.extend(new_chunks);
            state.update_file_chunks(&file_str, updated_chunks, modified);

            for chunk in removed_chunks {
                chroma::delete_chunk(client, &chunk.id).await?;
            }
        }

        pb.inc(1);
    }

    state.save(&config_dir)?;
    pb.finish_with_message("🎉 Indexing complete.");
    Ok(())
}

pub async fn handle_query(
    client: &Client,
    query: &str,
    top_k: usize,
    format: &str,
) -> anyhow::Result<()> {
    let embedding = embedding::get_embedding(&client, &query).await?;
    let parsed = chroma::query_chroma(&client, &embedding, top_k).await?;

    let docs = parsed["documents"]
        .as_array()
        .and_then(|outer| outer.get(0))
        .and_then(|inner| inner.as_array())
        .ok_or_else(|| anyhow::anyhow!("No documents in response"))?;

    let metas = parsed["metadatas"]
        .as_array()
        .and_then(|outer| outer.get(0))
        .and_then(|inner| inner.as_array())
        .ok_or_else(|| anyhow::anyhow!("No metadatas in response"))?;

    let dists = parsed["distances"]
        .as_array()
        .and_then(|outer| outer.get(0))
        .and_then(|inner| inner.as_array())
        .ok_or_else(|| anyhow::anyhow!("No distances in response"))?;

    let results: Vec<SearchResult> = docs
        .iter()
        .enumerate()
        .map(|(i, doc)| {
            let text = doc.as_str().unwrap_or("<invalid UTF-8>");
            let source = metas[i]
                .get("source")
                .and_then(|v| v.as_str())
                .unwrap_or("<unknown>");
            let distance = dists[i].as_f64().unwrap_or_default();

            SearchResult {
                index: i + 1,
                source,
                distance,
                content: text,
            }
        })
        .collect();

    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&results)?),
        "markdown" => {
            for r in &results {
                let lang = Path::new(r.source)
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("text");
                println!("### Result {}\n", r.index);
                println!("**Source:** `{}`  ", r.source);
                println!("**Distance:** `{:.4}`  ", r.distance);
                println!("```{}\n{}\n```", lang, r.content);
                println!();
            }
        }
        "smart" => {
            let context_chunks: Vec<String> = results.iter()
                .map(|r| {
                    let lang = Path::new(r.source)
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("text");

                    format!(
                        "**File:** `{}`\n\n```{}\n{}\n```",
                        r.source, lang, r.content
                    )
                })
                .collect();

            let raw_answer = llm::get_llm_response(client, query, &context_chunks).await?;
            let rendered = utils::render_markdown_highlighted(&raw_answer);

            println!("💡 Answer:\n\n{}", rendered);
        }
        _ => {
            for r in &results {
                println!("--- Result {} ---", r.index);
                println!("📄 Source: {}", r.source);
                println!("🔎 Distance: {:.4}", r.distance);
                println!("{}", utils::highlight_syntax(r.content, r.source));
                println!();
            }
        }
    }

    Ok(())
}
