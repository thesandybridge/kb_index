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
        /// Output format (pretty, json, markdown, smart)
        #[arg(short, long, default_value = "smart")]
        format: String,
        /// Session ID or 'new' to create a new session
        #[arg(long)]
        session: Option<String>,
    },
    /// Manage sessions for conversation history
    Sessions {
        /// List all available sessions
        #[arg(short, long, default_value_t = false)]
        list: bool,
        /// Clear the current session
        #[arg(short, long, default_value_t = false)]
        clear: bool,
        /// Switch to a specific session by ID
        #[arg(short, long)]
        switch: Option<String>,
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

