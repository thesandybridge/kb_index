use crate::config;
use std::fs;

pub fn handle_config(set_api_key: Option<String>, show: bool) -> anyhow::Result<()> {
    let config_path = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Unable to determine config directory"))?
        .join("kb-index")
        .join("config.toml");

    // Load existing config
    let mut config = config::load_config()?;

    // Set API key if provided
    if let Some(api_key) = set_api_key {
        config.openai_api_key = Some(api_key);

        // Save updated config
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(&config)?;
        fs::write(&config_path, content)?;

        println!("✅ API key saved to config file: {}", config_path.display());
    }

    // Show config if requested
    if show {
        println!("Configuration file: {}", config_path.display());
        println!("Chroma host: {}", config.chroma_host);
        println!("OpenAI API key: {}", match config.openai_api_key {
            Some(key) if !key.is_empty() => "Set (hidden for security)",
            _ => "Not set"
        });
        println!("OpenAI Completion Model: {}", config.openai_completion_model);
        println!("OpenAI Embedding Model: {}", config.openai_embedding_model);
        println!("Supported Extensions: {:?}", config.file_extensions.unwrap());
        println!("Syntax Theme: {:?}", config.syntax_theme.unwrap());
        // Check environment variable
        match std::env::var("OPENAI_API_KEY") {
            Ok(_) => println!("OPENAI_API_KEY environment variable: Set (hidden for security)"),
            Err(_) => println!("OPENAI_API_KEY environment variable: Not set"),
        }
    }

    Ok(())
}
