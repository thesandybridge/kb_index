use dirs::config_dir;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;

#[derive(Deserialize, Serialize, Clone)]
pub struct AppConfig {
    pub chroma_host: String,
    pub openai_api_key: Option<String>,
    pub openai_completion_model: String,
    pub openai_embedding_model: String,
    pub file_extensions: Option<Vec<String>>,
    pub syntax_theme: Option<String>,
}


pub fn default_extensions() -> Vec<String> {
    vec!["md", "rs", "tsx", "ts", "js", "jsx", "html"]
        .into_iter()
        .map(String::from)
        .collect()
}

pub fn load_config() -> anyhow::Result<AppConfig> {
    let config_path = config_dir()
        .ok_or_else(|| anyhow::anyhow!("Unable to determine config directory"))?
        .join("kb-index")
        .join("config.toml");

    let env_api_key = env::var("OPENAI_API_KEY").ok();

    if !config_path.exists() {
        let default = AppConfig {
            chroma_host: "http://localhost:8000".into(),
            openai_api_key: env_api_key,
            openai_completion_model: "gpt-4".to_string(),
            openai_embedding_model: "text-embedding-3-large".to_string(),
            file_extensions: Some(default_extensions()),
            syntax_theme: Some("gruvbox-dark".to_string()),
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

    if env_api_key.is_some() {
        config.openai_api_key = env_api_key;
    }

    Ok(config)
}

pub fn get_openai_api_key() -> anyhow::Result<String> {
    match env::var("OPENAI_API_KEY") {
        Ok(key) => {
            if !key.is_empty() {
                return Ok(key);
            }
        },
        Err(e) => {
            eprintln!("Debug: OPENAI_API_KEY environment variable error: {:?}", e);
        }
    }

    let config = load_config()?;
    if let Some(key) = config.openai_api_key {
        if !key.is_empty() {
            return Ok(key);
        }
    }

    anyhow::bail!("OpenAI API key not found in environment or config file. Please set the OPENAI_API_KEY environment variable or add it to your config file.")
}
