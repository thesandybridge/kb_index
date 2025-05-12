use crate::chroma;
use crate::config;
use crate::embedding;
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
