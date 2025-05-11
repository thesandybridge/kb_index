use crate::config;
use crate::state::{QueryState, hash_query_context};
use reqwest::Client;

pub async fn get_llm_response(
    client: &Client,
    prompt: &str,
    context_chunks: &[String],
) -> anyhow::Result<String> {
    let api_key = config::get_openai_api_key()?;
    let cfg = config::load_config()?;
    let config_dir = config::get_config_dir()?;
    let mut state = QueryState::load(&config_dir)?;

    // Generate context hash from context chunks
    let context_hash = hash_query_context(prompt, context_chunks);

    // Return cached answer if it exists
    if let Some(cached) = state.get_cached_answer(prompt, &context_hash) {
        return Ok(cached);
    }

    // Concatenate all context chunks for the prompt
    let full_context = context_chunks.join("\n\n---\n\n");

    let full_prompt = format!(
        "You are an expert personal and code assistant.\n\
         Use the following code snippets to answer the question. \
         Format your response in Markdown and include code where necessary.\n\n\
         Question:\n{}\n\nContext:\n{}\n\nAnswer:",
        prompt, full_context
    );

    let body = serde_json::json!({
        "model": cfg.openai_completion_model,
        "messages": [
            { "role": "system", "content": "You are an expert personal and code assistant." },
            { "role": "user", "content": full_prompt }
        ],
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

    // Cache the answer
    state.insert_answer(prompt.to_string(), context_hash, answer.clone());
    state.save(&config_dir)?;

    Ok(answer)
}
