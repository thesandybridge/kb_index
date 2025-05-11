use crate::config;
use indicatif::ProgressBar;
use reqwest::Client;
use serde::Serialize;
use std::path::Path;

pub const TENANT: &str = "default_tenant";
pub const DATABASE: &str = "default_database";
pub const COLLECTION: &str = "kb_index";

#[derive(Serialize)]
pub struct ChromaV2AddRequest {
    documents: Vec<String>,
    ids: Vec<String>,
    embeddings: Vec<Vec<f32>>,
    metadatas: Vec<serde_json::Value>,
}

#[derive(Serialize)]
pub struct SearchResult<'a> {
    pub index: usize,
    pub source: &'a str,
    pub distance: f64,
    pub content: &'a str,
}

pub async fn get_collection_id(client: &Client) -> anyhow::Result<String> {
    let config = config::load_config()?;
    let url = format!(
        "{}/api/v2/tenants/{}/databases/{}/collections",
        config.chroma_host, TENANT, DATABASE
    );

    let resp = client.get(&url).send().await?;
    let status = resp.status();
    let body = resp.text().await?;

    if !status.is_success() {
        println!("❌ Failed to list collections: HTTP {} - {}", status, body);
        anyhow::bail!("Failed to fetch collections");
    }

    let collections: serde_json::Value = serde_json::from_str(&body)?;
    if let Some(arr) = collections.as_array() {
        for collection in arr {
            if collection.get("name") == Some(&serde_json::Value::String(COLLECTION.to_string())) {
                if let Some(id) = collection.get("id").and_then(|v| v.as_str()) {
                    return Ok(id.to_string());
                }
            }
        }
    }

    anyhow::bail!("Collection '{}' not found", COLLECTION)
}

pub async fn create_collection_if_missing(client: &Client) -> anyhow::Result<()> {
    let config = config::load_config()?;
    let url = format!(
        "{}/api/v2/tenants/{}/databases/{}/collections",
        config.chroma_host, TENANT, DATABASE
    );

    let payload = serde_json::json!({
        "name": COLLECTION,
        "embedding_function": {
            "type": "openai",
            "model": "text-embedding-3-large"
        }
    });

    let resp = client.post(&url).json(&payload).send().await?;

    match resp.status() {
        reqwest::StatusCode::CONFLICT => Ok(()),
        status if status.is_success() => {
            println!("✅ Created collection '{}'", COLLECTION);
            Ok(())
        }
        status => {
            let body = resp.text().await?;
            println!("❌ Failed to create collection: HTTP {} - {}", status, body);
            anyhow::bail!("Failed to create collection")
        }
    }
}

pub async fn send_to_chroma(
    client: &Client,
    id: &str,
    doc: &str,
    embedding: &Vec<f32>,
    path: &Path,
    pb: &ProgressBar,
) -> anyhow::Result<()> {
    let config = config::load_config()?;
    create_collection_if_missing(&client).await?;
    let collection_id = get_collection_id(&client).await?;

    let payload = ChromaV2AddRequest {
        ids: vec![id.to_string()],
        embeddings: vec![embedding.clone()],
        documents: vec![doc.to_string()],
        metadatas: vec![serde_json::json!({
            "source": path.display().to_string()
        })],
    };

    let add_url = format!(
        "{}/api/v2/tenants/{}/databases/{}/collections/{}/add",
        config.chroma_host, TENANT, DATABASE, collection_id
    );

    let resp = client.post(&add_url).json(&payload).send().await?;
    let status = resp.status();
    let body = resp.text().await?;

    if !status.is_success() {
        pb.println(format!(
            "❌ Chroma error: HTTP {} - {}\nPayload ID: {}, Path: {}",
            status,
            body,
            id,
            path.display()
        ));
        anyhow::bail!("Failed to insert into Chroma");
    }

    pb.set_message(format!(
        "✅ Indexed chunk: file={}, chars={}",
        path.display(),
        doc.len()
    ));

    Ok(())
}

pub async fn query_chroma(
    client: &Client,
    embedding: &Vec<f32>,
    top_k: usize
) -> anyhow::Result<serde_json::Value> {
    let collection_id = get_collection_id(&client).await?;
    let config = config::load_config()?;

    let url = format!(
        "{}/api/v2/tenants/{}/databases/{}/collections/{}/query",
        config.chroma_host, TENANT, DATABASE, collection_id
    );

    let payload = serde_json::json!({
        "query_embeddings": [embedding],
        "n_results": top_k
    });

    let resp = client.post(&url).json(&payload).send().await?;
    let body = resp.text().await?;
    let parsed: serde_json::Value = serde_json::from_str(&body)?;

    Ok(parsed)
}

pub async fn delete_chunk(client: &Client, id: &str) -> anyhow::Result<()> {
    let config = config::load_config()?;
    let collection_id = get_collection_id(client).await?;

    let url = format!(
        "{}/api/v2/tenants/{}/databases/{}/collections/{}/delete",
        config.chroma_host, TENANT, DATABASE, collection_id
    );

    let payload = serde_json::json!({
        "ids": [id]
    });

    let resp = client.post(&url).json(&payload).send().await?;
    let status = resp.status();
    let body = resp.text().await?;

    if !status.is_success() {
        anyhow::bail!("Failed to delete chunk {}: HTTP {} - {}", id, status, body);
    }

    Ok(())
}
