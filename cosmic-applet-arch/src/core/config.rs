//! Config for cosmic-applet-arch

use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

const CONFIG_FILE_NAME: &str = "config.toml";

#[derive(Deserialize, Serialize, Debug, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields, default)]
pub struct Config {
    /// UpdateTypes to exclude from the updates count shown on the taskbar.
    /// These UpdateTypes are still checked and can be seen by opening the
    /// popup. See https://github.com/nick42d/cosmic-applet-arch/issues/28
    pub exclude_from_counter: HashSet<UpdateType>,
    /// How often to compare current packages with the latest version in memory.
    pub interval_secs: u64,
    /// How long the api call can run without triggering a timeout.
    pub timeout_secs: u64,
    /// Every `online_check_period` number of `interval_secs`s (starting at the
    /// first interval), the system will update the latest version in memory
    /// from the internet.
    pub online_check_period: usize,
    /// If you are using unofficial repositories, a package url can be provided.
    pub other_repo_urls: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Hash, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum UpdateType {
    Aur,
    Devel,
    Pacman,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            exclude_from_counter: Default::default(),
            interval_secs: 6,
            timeout_secs: 120,
            online_check_period: 600,
            other_repo_urls: Default::default(),
        }
    }
}

pub async fn get_config() -> anyhow::Result<Config> {
    let dirs = super::proj_dirs().context("Unable to obtain a local config directory")?;
    let config_dir = dirs.config_dir();
    tokio::fs::create_dir_all(config_dir)
        .await
        .context("Unable to create config directory")?;
    let mut config_file_path = config_dir.to_path_buf();
    config_file_path.push(CONFIG_FILE_NAME);

    // Use default config if there is no config file.
    match tokio::fs::read_to_string(config_file_path).await {
        Ok(file) => toml::from_str(&file).context("Invalid config file"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
        Err(e) => Err(e).context("IO error reading config file"),
    }
}

#[cfg(test)]
mod tests {
    use super::UpdateType;
    use crate::core::config::Config;
    use serde::Deserialize;

    #[tokio::test]
    async fn test_config_reads() {
        let file = tokio::fs::read_to_string("config/config.toml")
            .await
            .unwrap();
        let parsed = toml::from_str::<Config>(&file).unwrap();
        assert_eq!(parsed, Config::default())
    }
    #[tokio::test]
    async fn test_config_custom_exclude_from_counter() {
        let example = toml::toml! {
            exclude_from_counter = ["aur", "devel"]
        };
        let actual = Config::deserialize(example).unwrap();
        let expected = Config {
            exclude_from_counter: [UpdateType::Aur, UpdateType::Devel].into(),
            ..Default::default()
        };
        assert_eq!(actual, expected)
    }
    #[tokio::test]
    async fn test_config_custom_other_repo_urls() {
        let example = toml::toml! {
            other_repo_urls.endeavouros = "https://github.com/endeavouros-team/PKGBUILDS/tree/master/{pkgname}"
            other_repo_urls.chaotic-aur = "https://gitlab.com/chaotic-aur/pkgbuilds/-/tree/main/{pkgname}"
        };
        let actual = Config::deserialize(example).unwrap();
        let expected = Config {
            other_repo_urls: [
                (
                    "endeavouros".into(),
                    "https://github.com/endeavouros-team/PKGBUILDS/tree/master/{pkgname}".into(),
                ),
                (
                    "chaotic-aur".into(),
                    "https://gitlab.com/chaotic-aur/pkgbuilds/-/tree/main/{pkgname}".into(),
                ),
            ]
            .into(),
            ..Default::default()
        };
        assert_eq!(actual, expected)
    }
}
