use crate::engine::types::TypingConfig;
use anyhow::Result;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub snippets: HashMap<String, String>,
    pub typing_config: TypingConfig,
    pub last_window: Option<String>,
    pub window_position: Option<[f32; 2]>,
    pub window_size: Option<[f32; 2]>,
}

impl AppConfig {
    pub fn config_path() -> Result<PathBuf> {
        let proj_dirs = ProjectDirs::from("com", "kraken", "kraken-rs")
            .ok_or_else(|| anyhow::anyhow!("Could not find config directory"))?;
        let config_dir = proj_dirs.config_dir();
        fs::create_dir_all(config_dir)?;
        Ok(config_dir.join("config.json"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if path.exists() {
            let content = fs::read_to_string(&path)?;
            let config: AppConfig = serde_json::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }

    pub fn add_snippet(&mut self, name: String, content: String) -> Result<()> {
        self.snippets.insert(name, content);
        self.save()
    }

    pub fn remove_snippet(&mut self, name: &str) -> Result<()> {
        self.snippets.remove(name);
        self.save()
    }

    pub fn get_snippet(&self, name: &str) -> Option<&String> {
        self.snippets.get(name)
    }

    pub fn snippet_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.snippets.keys().cloned().collect();
        names.sort();
        names
    }
}
