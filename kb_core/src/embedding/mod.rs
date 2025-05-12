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

pub async fn get_embeddings(client: &Client, texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>> {
    let config = config::load_config()?;
    let api_key = config::get_openai_api_key()?;

    let body = serde_json::json!({
        "model": config.openai_embedding_model,
        "input": texts
    });

    let res = client
        .post("https://api.openai.com/v1/embeddings")
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await?;

    let status = res.status();
    let text_body = res.text().await?;

    if !status.is_success() {
        eprintln!("❌ OpenAI error: HTTP {} - {}", status, text_body);
        anyhow::bail!("Embedding batch failed");
    }

    let parsed: serde_json::Value = serde_json::from_str(&text_body)?;
    let data = parsed["data"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Invalid embedding response format"))?;

    Ok(data.iter()
        .map(|v| {
            v["embedding"]
                .as_array()
                .unwrap_or(&vec![])
                .iter()
                .map(|f| f.as_f64().unwrap_or_default() as f32)
                .collect()
        })
        .collect())
}

pub async fn get_embedding(client: &Client, text: &str) -> anyhow::Result<Vec<f32>> {
    let config = config::load_config()?;
    let body = EmbeddingRequest {
        input: vec![text.to_string()],
        model: config.openai_embedding_model.into(),
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
