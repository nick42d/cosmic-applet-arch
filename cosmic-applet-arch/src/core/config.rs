//! Config for cosmic-applet-arch

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

const CONFIG_FILE_NAME: &str = "config.toml";

#[derive(Deserialize, Serialize, Debug, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct Config {
    /// UpdateTypes to exclude from the updates count shown on the taskbar.
    /// These UpdateTypes are still checked and can be seen by opening the
    /// popup. See https://github.com/nick42d/cosmic-applet-arch/issues/28
    exclude_from_counter: HashSet<UpdateType>,
    /// How often to compare current packages with the latest version in memory.
    interval_secs: u64,
    /// How long the api call can run without triggering a timeout.
    timeout_secs: u64,
    /// Every `online_check_period` number of `interval_secs`s (starting at the
    /// first interval), the system will update the latest version in memory
    /// from the internet.
    online_check_period: usize,
    /// If you are using unofficial repositories, a package url can be provided.
    other_repo_urls: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Hash)]
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

async fn get_config() -> Result<Config, std::io::Error> {
    let dirs = super::proj_dirs().unwrap();
    let config_dir = dirs.config_dir();
    tokio::fs::create_dir_all(config_dir).await.unwrap();
    let mut config_file_path = config_dir.to_path_buf();
    config_file_path.push(CONFIG_FILE_NAME);
    let file = tokio::fs::read_to_string(config_file_path).await.unwrap();
    Ok(toml::from_str(&file).unwrap())
}

#[cfg(test)]
mod tests {
    use crate::core::config::Config;

    #[tokio::test]
    async fn test_config_reads() {
        let file = tokio::fs::read_to_string("test/config.toml").await.unwrap();
        let parsed = toml::from_str::<Config>(&file).unwrap();
        let mut expeceted = Config {
            exclude_from_counter: todo!(),
            interval_secs: todo!(),
            timeout_secs: todo!(),
            online_check_period: todo!(),
            other_repo_urls: todo!(),
        };
        assert_eq!(parsed, Config::default())
    }
}
