use kb_core::cli;
use kb_core::config;

use cli::{commands, Cli};
use clap::Parser;
use reqwest::Client;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let client = Client::new();

    match cli {
        Cli::Config { set_api_key, show } => {
            // Config command doesn't need the API key validation
            return commands::config::handle_config(set_api_key, show);
        }
        _ => {
            // For other commands, validate that we have an OpenAI API key before proceeding
            match config::get_openai_api_key() {
                Ok(_) => {}, // Key exists, continue
                Err(e) => {
                    eprintln!("Error: {}", e);
                    eprintln!("\nTo fix this issue, either:");
                    eprintln!("1. Set the OPENAI_API_KEY environment variable");
                    eprintln!("   export OPENAI_API_KEY=your_api_key_here");
                    eprintln!("2. Add your API key to the config file at:");
                    if let Ok(config) = config::load_config() {
                        if let Some(path) = dirs::config_dir() {
                            let config_path = path.join("kb-index").join("config.toml");
                            eprintln!("   {}", config_path.display());
                            eprintln!("\nExample config.toml:");
                            eprintln!("chroma_host = \"{}\"", config.chroma_host);
                            eprintln!("openai_api_key = \"your_api_key_here\"");
                            eprintln!("\nOr use the config command to set your API key:");
                            eprintln!("   kb-index config --set-api-key=\"your_api_key_here\"");
                        }
                    }
                    return Err(e);
                }
            }
        }
    }

    match cli {
        Cli::Index { path } => {
            commands::index::handle_index(&client, &path).await?;
        }
        Cli::Query { query, top_k, format, session} => {
            commands::query::handle_query(&client, &query, top_k, &format, session).await?;
        }
        Cli::Sessions { list, clear, switch } => {
            commands::session::handle_sessions(list, clear, switch)?;
        }
        _ => {} // Config case already handled above
    }

    Ok(())
}
