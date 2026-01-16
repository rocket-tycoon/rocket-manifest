use anyhow::{Context, Result};
use dirs::config_dir;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const APP_NAME: &str = "manifest-app";
const CONFIG_FILE: &str = "config.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Width of the main window
    pub window_width: Option<f32>,
    /// Height of the main window
    pub window_height: Option<f32>,
    /// Width of the feature panel (left sidebar)
    pub feature_panel_width: Option<f32>,
    /// Ratio of the editor height in the vertical editor/terminal split (0.0 to 1.0).
    /// Default is 0.6 (60% editor, 40% terminal).
    pub editor_split_ratio: f32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            window_width: Some(1200.0),
            window_height: Some(800.0),
            feature_panel_width: Some(250.0),
            editor_split_ratio: 0.6,
        }
    }
}

impl AppConfig {
    /// Load configuration from the user's config directory.
    /// Returns default config if file doesn't exist or fails to parse.
    pub fn load() -> Self {
        match Self::try_load() {
            Ok(config) => config,
            Err(e) => {
                eprintln!("Failed to load config, using defaults: {}", e);
                Self::default()
            }
        }
    }

    fn try_load() -> Result<Self> {
        let config_path = get_config_path()?;
        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&config_path).context("Failed to read config file")?;

        let config = serde_json::from_str(&content).context("Failed to parse config file")?;

        Ok(config)
    }

    /// Save the current configuration to disk.
    pub fn save(&self) -> Result<()> {
        let config_path = get_config_path()?;

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).context("Failed to create config directory")?;
        }

        let content = serde_json::to_string_pretty(self).context("Failed to serialize config")?;

        fs::write(&config_path, content).context("Failed to write config file")?;

        Ok(())
    }
}

fn get_config_path() -> Result<PathBuf> {
    let mut path =
        config_dir().ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;
    path.push(APP_NAME);
    path.push(CONFIG_FILE);
    Ok(path)
}
