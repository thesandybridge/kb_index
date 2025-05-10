use clap::Parser;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use syntect::easy::HighlightLines;
use syntect::highlighting::Style;
use syntect::parsing::SyntaxSet;
use syntect::util::{as_24_bit_terminal_escaped, LinesWithEndings};
use two_face::theme::extra;
use uuid::Uuid;
use walkdir::WalkDir;

#[derive(Parser)]
#[command(name = "kb-index")]
#[command(about = "Index or query local files using Chroma")]
enum Cli {
    Index {
        path: PathBuf,
    },
    Query {
        query: String,
        #[arg(short, long, default_value_t = 5)]
        top_k: usize,
        #[arg(short, long, default_value = "pretty")]
        format: String,
    },
}

#[derive(Serialize)]
struct SearchResult<'a> {
    index: usize,
    source: &'a str,
    distance: f64,
    content: &'a str,
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

        Cli::Query {
            query,
            top_k,
            format,
        } => {
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

            let mut results = vec![];

            for (i, doc) in docs.iter().enumerate() {
                let text = doc.as_str().unwrap_or("<invalid UTF-8>");
                let source = metas[i]
                    .get("source")
                    .and_then(|v| v.as_str())
                    .unwrap_or("<unknown>");
                let distance = dists[i].as_f64().unwrap_or_default();

                results.push(SearchResult {
                    index: i + 1,
                    source,
                    distance,
                    content: text,
                });
            }

            match format.as_str() {
                "json" => {
                    println!("{}", serde_json::to_string_pretty(&results)?);
                }
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
                _ => {
                    for r in &results {
                        println!("--- Result {} ---", r.index);
                        println!("📄 Source: {}", r.source);
                        println!("🔎 Distance: {:.4}", r.distance);
                        println!("{}", highlight_syntax(r.content, r.source));
                        println!();
                    }
                }
            }
        }
    }

    Ok(())
}

fn highlight_syntax(code: &str, file_path: &str) -> String {
    let ps = SyntaxSet::load_defaults_newlines();

    // Load GruvboxDark theme from two-face
    let theme_set = extra(); // loads extended themes

    let theme = theme_set.get(two_face::theme::EmbeddedThemeName::GruvboxDark);

    let syntax = ps
        .find_syntax_for_file(file_path)
        .ok()
        .flatten()
        .unwrap_or_else(|| ps.find_syntax_plain_text());

    let mut h = HighlightLines::new(syntax, theme);
    let mut result = String::new();

    for line in LinesWithEndings::from(code) {
        let ranges: Vec<(Style, &str)> = h.highlight_line(line, &ps).unwrap();
        result.push_str(&as_24_bit_terminal_escaped(&ranges[..], false));
    }

    result
}

fn collect_files(root: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if root.is_file() {
        files.push(root.to_path_buf());
    } else {
        for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            if path.is_file()
                && matches!(
                    path.extension().and_then(|s| s.to_str()),
                    Some("md" | "rs" | "tsx" | "ts" | "js" | "jsx")
                )
            {
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
        model: "text-embedding-3-large".into(),
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

async fn send_to_chroma(
    client: &Client,
    id: &str,
    doc: &str,
    embedding: &Vec<f32>,
    path: &Path,
) -> anyhow::Result<()> {
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
        "http://192.168.30.7:8000/api/v2/tenants/{}/databases/{}/collections/{}/add",
        TENANT, DATABASE, collection_id
    );

    let resp = client.post(&add_url).json(&payload).send().await?;
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
