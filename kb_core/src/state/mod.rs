use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::io::Write;

use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use anyhow::{Result, Context};

const INDEX_STATE_FILE: &str = "index-state.json";
const QUERY_CACHE_FILE: &str = "query-cache.json";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IndexedChunk {
    pub hash: String,
    pub id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileMetadata {
    pub last_modified: u64,
    pub chunks: Vec<IndexedChunk>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct IndexState {
    pub files: HashMap<String, FileMetadata>,
}

impl IndexState {
    pub fn load(config_dir: &PathBuf) -> Result<Self> {
        let path = config_dir.join(INDEX_STATE_FILE);
        if !path.exists() {
            return Ok(IndexState::default());
        }
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read index state from {}", path.display()))?;
        let state = serde_json::from_str(&contents)?;
        Ok(state)
    }

    pub fn save(&self, config_dir: &PathBuf) -> Result<()> {
        let path = config_dir.join(INDEX_STATE_FILE);
        let json = serde_json::to_string_pretty(self)?;
        let mut file = fs::File::create(&path)?;
        file.write_all(json.as_bytes())?;
        Ok(())
    }

    pub fn get_file_chunks(&self, path: &str) -> Option<&Vec<IndexedChunk>> {
        self.files.get(path).map(|meta| &meta.chunks)
    }

    pub fn get_last_modified(&self, path: &str) -> Option<u64> {
        self.files.get(path).map(|meta| meta.last_modified)
    }

    pub fn update_file_chunks(&mut self, path: &str, chunks: Vec<IndexedChunk>, last_modified: u64) {
        self.files.insert(
            path.to_string(),
            FileMetadata {
                last_modified,
                chunks,
            },
        );
    }

    pub fn hash_chunk(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        hex::encode(hasher.finalize())
    }

    pub fn has_chunk(state: &[IndexedChunk], hash: &str) -> bool {
        state.iter().any(|chunk| chunk.hash == hash)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QueryCache {
    pub query: String,
    pub context_hash: String,
    pub embedding: Vec<f32>,
    pub answer: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct QueryState {
    pub entries: Vec<QueryCache>,
}

impl QueryState {
    pub fn load(config_dir: &PathBuf) -> Result<Self> {
        let path = config_dir.join(QUERY_CACHE_FILE);
        if !path.exists() {
            return Ok(QueryState::default());
        }
        let contents = fs::read_to_string(&path)?;
        let state = serde_json::from_str(&contents)?;
        Ok(state)
    }

    pub fn save(&self, config_dir: &PathBuf) -> Result<()> {
        let path = config_dir.join(QUERY_CACHE_FILE);
        let json = serde_json::to_string_pretty(self)?;
        let mut file = fs::File::create(&path)?;
        file.write_all(json.as_bytes())?;
        Ok(())
    }

    pub fn get_cached_answer(&self, query: &str, context_hash: &str) -> Option<String> {
        self.entries.iter()
            .find(|e| e.query == query && e.context_hash == context_hash)
            .map(|e| e.answer.clone())
    }

    pub fn insert_answer(
        &mut self,
        query: String,
        context_hash: String,
        embedding: Vec<f32>,
        answer: String
    ) {
        self.entries.push(QueryCache { query, context_hash, embedding, answer });
    }

    pub fn find_similar(&self, query_embedding: &[f32], threshold: f32) -> Option<String> {
        self.entries
            .iter()
            .filter_map(|e| {
                if e.embedding.len() != query_embedding.len() {
                    return None;
                }

                let similarity = cosine_similarity(&e.embedding, query_embedding);
                if similarity > threshold {
                    Some((similarity, e.answer.clone()))
                } else {
                    None
                }
            })
            .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
            .map(|(_, answer)| answer)
    }
}

pub fn hash_query_context(query: &str, context_chunks: &[String]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(query.as_bytes());
    for chunk in context_chunks {
        hasher.update(chunk.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum::<f32>();
    let norm_a = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot / (norm_a * norm_b + 1e-8) // Add small epsilon to avoid div-by-zero
}
