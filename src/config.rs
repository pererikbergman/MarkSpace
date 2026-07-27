//! Persistent app configuration: the recent-workspace registry stored as TOML
//! at `~/.config/markspace/config.toml` (PRD §3.2, §6; keys per ADR 0002).
//!
//! Pure load/save logic with no dependency on egui. The path is injected so
//! tests can use a temp file; the glue supplies the default path.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The on-disk config: the ordered list of recent workspace roots.
#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub workspaces: Vec<PathBuf>,
}

impl Config {
    /// Load the config from `path`. A missing or malformed file yields an empty
    /// config rather than an error, so a bad file never blocks startup.
    pub fn load(path: &Path) -> Config {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Write the config to `path` as TOML, creating parent directories.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let config = Config {
            workspaces: vec![PathBuf::from("/w/notes"), PathBuf::from("/w/docs")],
        };

        config.save(&path).unwrap();
        let loaded = Config::load(&path);

        assert_eq!(loaded, config);
    }

    #[test]
    fn load_missing_file_is_empty() {
        let dir = tempdir().unwrap();
        let loaded = Config::load(&dir.path().join("does-not-exist.toml"));
        assert_eq!(loaded, Config::default());
    }

    #[test]
    fn load_malformed_file_is_empty() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "this is { not valid toml ][").unwrap();

        let loaded = Config::load(&path);

        assert_eq!(loaded, Config::default(), "bad file must not block startup");
    }
}
