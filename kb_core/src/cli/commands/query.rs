use crate::chroma::{self, SearchResult};
use crate::embedding;
use crate::llm;
use crate::utils;
use reqwest::Client;
use std::path::Path;
use crate::state::{QueryState, SessionManager, hash_query_context};
use crate::config;

pub async fn handle_query(
    client: &Client,
    query: &str,
    top_k: usize,
    format: &str,
    session_id: Option<String>,
) -> anyhow::Result<()> {
    let config_dir = config::get_config_dir()?;
    let mut cache = QueryState::load(&config_dir)?;
    let mut session_manager = SessionManager::load(&config_dir)?;

    // Handle session management
    if let Some(id) = session_id {
        if id == "new" {
            let new_id = session_manager.create_session();
            println!("🆕 Created new session: {}", new_id);
        } else {
            // Pass a reference to set_active_session
            session_manager.set_active_session(&id)?;
            println!("🔄 Switched to session: {}", id);
        }
    } else if session_manager.active_session.is_none() {
        // Create a default session if none exists
        let new_id = session_manager.create_session();
        println!("🆕 Created default session: {}", new_id);
    }


    // Embed the query
    let query_embedding = embedding::get_embedding(client, query).await?;

    // 🔍 Try similarity cache
    if let Some(similar) = cache.find_similar(&query_embedding, 0.93) {
        println!("💡 Cached Answer:\n\n{}", utils::render_markdown_highlighted(&similar));

        // Add to session history even if cached
        if let Some(session) = session_manager.get_active_session_mut() {
            session.queries.push(query.to_string());
            session.responses.push(similar);
            session_manager.save(&config_dir)?;
        }

        return Ok(());
    }

    // Otherwise: do Chroma vector search
    let parsed = chroma::query_chroma(client, &query_embedding, top_k).await?;

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

            let context_hash = hash_query_context(query, &context_chunks);

            // Pass session manager to get_llm_response
            let raw_answer = llm::get_llm_response(
                client,
                query,
                &context_chunks,
                Some(&session_manager)
            ).await?;

            let rendered = utils::render_markdown_highlighted(&raw_answer);

            // 🧠 Cache the answer with the current query embedding
            cache.insert_answer(query.to_string(), context_hash, query_embedding.clone(), raw_answer.clone());
            cache.save(&config_dir)?;

            // Add to session history
            session_manager.add_interaction(query.to_string(), raw_answer)?;
            session_manager.save(&config_dir)?;

            println!("💡 Answer:\n\n{}", rendered);

            if let Some(session) = session_manager.get_active_session() {
                println!("\n📝 Session: {} (Q&A: {})",
                    &session.id[..8],
                    session.queries.len()
                );
            }
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

