pub mod commands;

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "kb-index")]
#[command(about = "Index or query local files using Chroma")]
pub enum Cli {
    /// Index files at the specified path
    Index {
        /// Path to the file or directory to index
        path: PathBuf,
    },
    /// Query the index with a text prompt
    Query {
        /// The query text to search for
        query: String,
        /// Number of results to return
        #[arg(short, long, default_value_t = 5)]
        top_k: usize,
        /// Output format (pretty, json, markdown)
        #[arg(short, long, default_value = "pretty")]
        format: String,
    },
    /// Configure the application
    Config {
        /// Set the OpenAI API key
        #[arg(long)]
        set_api_key: Option<String>,
        /// Show current configuration
        #[arg(long, default_value_t = false)]
        show: bool,
    },
}
