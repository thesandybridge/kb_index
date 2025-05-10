use dirs::config_dir;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;

#[derive(Deserialize, Serialize, Clone)]
pub struct AppConfig {
    pub chroma_host: String,
    pub openai_api_key: Option<String>,
}

pub fn load_config() -> anyhow::Result<AppConfig> {
    let config_path = config_dir()
        .ok_or_else(|| anyhow::anyhow!("Unable to determine config directory"))?
        .join("kb-index")
        .join("config.toml");

    // Try to get API key from environment first
    let env_api_key = env::var("OPENAI_API_KEY").ok();

    if !config_path.exists() {
        let default = AppConfig {
            chroma_host: "http://192.168.30.7:8000".into(),
            openai_api_key: env_api_key,
        };

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(&default)?;
        fs::write(&config_path, content)?;

        println!("✅ Created default config at {}", config_path.display());
        return Ok(default);
    }

    let contents = fs::read_to_string(&config_path)?;
    let mut config: AppConfig = toml::from_str(&contents)?;
    
    // If API key is in environment, it takes precedence over config file
    if env_api_key.is_some() {
        config.openai_api_key = env_api_key;
    }
    
    Ok(config)
}

/// Get the OpenAI API key from config or environment
pub fn get_openai_api_key() -> anyhow::Result<String> {
    // First try to get from environment
    match env::var("OPENAI_API_KEY") {
        Ok(key) => {
            if !key.is_empty() {
                return Ok(key);
            }
        },
        Err(e) => {
            // Log the specific error for debugging
            eprintln!("Debug: OPENAI_API_KEY environment variable error: {:?}", e);
        }
    }
    
    // Then try from config file
    let config = load_config()?;
    if let Some(key) = config.openai_api_key {
        if !key.is_empty() {
            return Ok(key);
        }
    }
    
    anyhow::bail!("OpenAI API key not found in environment or config file. Please set the OPENAI_API_KEY environment variable or add it to your config file.")
}
