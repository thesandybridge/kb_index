use crate::config;
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct EmbeddingRequest {
    input: Vec<String>,
    model: String,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

pub async fn get_embedding(client: &Client, text: &str) -> anyhow::Result<Vec<f32>> {
    let body = EmbeddingRequest {
        input: vec![text.to_string()],
        model: "text-embedding-3-large".into(),
    };

    // Get API key from config or environment
    let api_key = config::get_openai_api_key()?;

    let response = client
        .post("https://api.openai.com/v1/embeddings")
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await?;

    let status = response.status();
    let text_body = response.text().await?;

    if !status.is_success() {
        println!("❌ OpenAI error: HTTP {} - {}", status, text_body);
        anyhow::bail!("OpenAI returned an error");
    }

    match serde_json::from_str::<EmbeddingResponse>(&text_body) {
        Ok(parsed) => Ok(parsed.data.into_iter().next().unwrap().embedding),
        Err(err) => {
            println!("❌ Failed to parse response JSON: {}", err);
            println!("Raw response:\n{}", text_body);
            Err(err.into())
        }
    }
}
