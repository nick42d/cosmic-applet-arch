//! Config for cosmic-applet-arch

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Deserialize, Serialize)]
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
}

#[derive(Deserialize, Serialize, Eq, PartialEq, Hash)]
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
        }
    }
}
