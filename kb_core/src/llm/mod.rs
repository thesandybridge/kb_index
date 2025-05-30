use crate::config;
use crate::embedding;
use crate::state::SessionManager;
use crate::state::{QueryState, hash_query_context};
use reqwest::Client;

pub async fn get_llm_response(
    client: &Client,
    prompt: &str,
    context_chunks: &[String],
    session_manager: Option<&SessionManager>,
) -> anyhow::Result<String> {
    let api_key = config::get_openai_api_key()?;
    let cfg = config::load_config()?;
    let config_dir = config::get_config_dir()?;
    let mut state = QueryState::load(&config_dir)?;

    let context_hash = hash_query_context(prompt, context_chunks);

    // Generate query embedding (for similarity + caching)
    let embedding = embedding::get_embedding(client, prompt).await?;

    if let Some(similar) = state.find_similar(&embedding, 0.93) {
        return Ok(similar);
    }

    // Check for cached similar answer
    if let Some(cached) = state.get_cached_answer(prompt, &context_hash) {
        return Ok(cached);
    }

    // Prepare full prompt with session context if available
    let full_context = context_chunks.join("\n\n---\n\n");

    let mut messages = vec![
        serde_json::json!({
            "role": "system",
            "content": "You are an expert personal and code assistant."
        }),
    ];

    // Add session context if available
    if let Some(manager) = session_manager {
        if let Some(session) = manager.get_active_session() {
            // Add previous interactions as context
            for (q, r) in session.queries.iter().zip(session.responses.iter()) {
                messages.push(serde_json::json!({
                    "role": "user",
                    "content": q
                }));

                messages.push(serde_json::json!({
                    "role": "assistant",
                    "content": r
                }));
            }
        }
    }

    // Add current query with context
    let user_content = format!(
        "Use the following code snippets to answer the question. \
         Format your response in Markdown and include code where necessary.\n\n\
         Question:\n{}\n\nContext:\n{}",
        prompt, full_context
    );

    messages.push(serde_json::json!({
        "role": "user",
        "content": user_content
    }));

    let body = serde_json::json!({
        "model": cfg.openai_completion_model,
        "messages": messages,
        "temperature": 0.4
    });

    let res = client
        .post("https://api.openai.com/v1/chat/completions")
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await?;

    let text = res.text().await?;
    let value: serde_json::Value = serde_json::from_str(&text)?;

    let answer = value["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("No answer generated")
        .to_string();

    state.insert_answer(prompt.to_string(), context_hash, embedding, answer.clone());
    state.save(&config_dir)?;

    Ok(answer)
}

