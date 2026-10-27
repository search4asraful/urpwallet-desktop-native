use std::path::PathBuf;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub database_path: Option<String>,
    pub export_path: Option<String>,
}

impl AppConfig {
    pub fn config_file_path() -> PathBuf {
        if let Some(proj_dirs) = ProjectDirs::from("com", "urpwallet", "urpwallet") {
            proj_dirs.config_dir().join("config.json")
        } else {
            PathBuf::from("data/config.json")
        }
    }

    pub fn load() -> Self {
        let path = Self::config_file_path();
        if let Ok(content) = std::fs::read_to_string(&path)
            && let Ok(cfg) = serde_json::from_str::<AppConfig>(&content)
        {
            return cfg;
        }
        Self::default()
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let path = Self::config_file_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }
}
