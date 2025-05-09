use clap::Parser;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use uuid::Uuid;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use regex::Regex;
use console::Style;

#[derive(Parser)]
#[command(name = "kb-index")]
#[command(about = "Index or query local files using Chroma")]
enum Cli {
    /// Index a file or directory
    Index {
        /// Path to file or directory
        path: PathBuf,
    },
    /// Query the index
    Query {
        /// Natural language query string
        query: String,

        /// Number of results to return
        #[arg(short, long, default_value_t = 5)]
        top_k: usize,
    },
}

#[derive(Serialize)]
struct EmbeddingRequest {
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

#[derive(Serialize)]
struct ChromaV2AddRequest {
    documents: Vec<String>,
    ids: Vec<String>,
    embeddings: Vec<Vec<f32>>,
    metadatas: Vec<serde_json::Value>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let client = Client::new();

    match cli {
        Cli::Index { path } => {
            let paths = collect_files(&path)?;
            for path in paths {
                let content = fs::read_to_string(&path)?;
                let chunks = chunk_text(&content);
                for chunk in chunks {
                    let id = Uuid::new_v4().to_string();
                    let embedding = get_embedding(&client, &chunk).await?;
                    send_to_chroma(&client, &id, &chunk, &embedding, &path).await?;
                }
            }
            println!("Indexing complete.");
        }

        Cli::Query { query, top_k } => {
            let embedding = get_embedding(&client, &query).await?;
            let collection_id = get_collection_id(&client).await?;

            let url = format!(
                "http://192.168.30.7:8000/api/v2/tenants/{}/databases/{}/collections/{}/query",
                TENANT, DATABASE, collection_id
            );

            let payload = serde_json::json!({
                "query_embeddings": [embedding],
                "n_results": top_k
            });

            let resp = client.post(&url).json(&payload).send().await?;

            let body = resp.text().await?;

            let parsed: serde_json::Value = serde_json::from_str(&body)?;

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

            for (i, doc) in docs.iter().enumerate() {
                let text = doc.as_str().unwrap_or("<invalid UTF-8>");
                println!("--- Result {} ---", i + 1);
                println!("📄 Source: {}", metas[i].get("source").unwrap_or(&serde_json::Value::String("<unknown>".into())));
                println!("🔎 Distance: {:.4}", dists[i].as_f64().unwrap_or_default());
                println!("{}", highlight_query(text, &query));
                println!();
            }
        }
    }

    Ok(())
}

fn highlight_query(text: &str, query: &str) -> String {
    let escaped = regex::escape(query);
    let re = Regex::new(&format!(r"(?i)\b{}\b", escaped)).unwrap(); // word-boundary, case-insensitive
    let highlight = Style::new().bold().yellow();

    re.replace_all(text, |caps: &regex::Captures| {
        highlight.apply_to(&caps[0]).to_string()
    }).to_string()
}

fn collect_files(root: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if root.is_file() {
        files.push(root.to_path_buf());
    } else {
        for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            if path.is_file() && matches!(path.extension().and_then(|s| s.to_str()), Some("md" | "rs" | "tsx" | "js" | "jsx")) {
                files.push(path.to_path_buf());
            }
        }
    }
    Ok(files)
}

fn chunk_text(text: &str) -> Vec<String> {
    text.lines()
        .collect::<Vec<_>>()
        .chunks(10)
        .map(|chunk| chunk.join("\n"))
        .collect()
}

async fn get_embedding(client: &Client, text: &str) -> anyhow::Result<Vec<f32>> {
    let body = EmbeddingRequest {
        input: vec![text.to_string()],
        model: "text-embedding-3-small".into(),
    };

    let response = client
        .post("https://api.openai.com/v1/embeddings")
        .bearer_auth(std::env::var("OPENAI_API_KEY")?)
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

const TENANT: &str = "default_tenant";
const DATABASE: &str = "default_database";
const COLLECTION: &str = "kb_index";

async fn get_collection_id(client: &Client) -> anyhow::Result<String> {
    let url = format!(
        "http://192.168.30.7:8000/api/v2/tenants/{}/databases/{}/collections",
        TENANT, DATABASE
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

async fn create_collection_if_missing(client: &Client) -> anyhow::Result<()> {
    let url = format!(
        "http://192.168.30.7:8000/api/v2/tenants/{}/databases/{}/collections",
        TENANT, DATABASE
    );

    let payload = serde_json::json!({
        "name": COLLECTION,
        "embedding_function": {
        "type": "openai",
        "model": "text-embedding-3-small"
        }
    });

    let resp = client.post(&url).json(&payload).send().await?;

    match resp.status() {
        reqwest::StatusCode::CONFLICT => {
            // already exists — fine
            Ok(())
        }
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

async fn send_to_chroma(
    client: &Client,
    id: &str,
    doc: &str,
    embedding: &Vec<f32>,
    path: &Path,
) -> anyhow::Result<()> {
    create_collection_if_missing(&client).await?;
    let collection_id = get_collection_id(&client).await?;

    // Build payload
    let payload = ChromaV2AddRequest {
        ids: vec![id.to_string()],
        embeddings: vec![embedding.clone()],
        documents: vec![doc.to_string()],
        metadatas: vec![serde_json::json!({
            "source": path.display().to_string()
        })],
    };

    let add_url = format!(
        "http://192.168.30.7:8000/api/v2/tenants/{}/databases/{}/collections/{}/add",
        TENANT, DATABASE, collection_id
    );

    let resp = client
        .post(&add_url)
        .json(&payload)
        .send()
    .await?;

    let status = resp.status();
    let body = resp.text().await?;

    if !status.is_success() {
        println!(
            "❌ Chroma error: HTTP {} - {}\nPayload ID: {}, Path: {}",
            status,
            body,
            id,
            path.display()
        );
        anyhow::bail!("Failed to insert into Chroma");
    } else {
        println!(
            "✅ Added chunk to Chroma: file={}, id={}, chars={}",
            path.display(),
            id,
            doc.len()
        );
    }

    Ok(())
}
