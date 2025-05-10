use reqwest::Client;
use crate::config;

pub async fn get_llm_response(client: &Client, prompt: &str) -> anyhow::Result<String> {
    let api_key = config::get_openai_api_key()?;

    let body = serde_json::json!({
        "model": "gpt-4",
        "messages": [
            { "role": "system", "content": "You are a expert personal and code assistant." },
            { "role": "user", "content": prompt }
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

    Ok(answer)
}
