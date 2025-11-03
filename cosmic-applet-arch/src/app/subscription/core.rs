// NOTE: Conditional compilation has been used in this module to allow the
// applet to be populated with mock data for testing.
//
// Development should be done considering that developers will have all features
// enabled - ie unused code warnings triggered when `mock-api` feature is
// enabled should be ignored.

use crate::core::proj_dirs;
use crate::news::{DatedNewsItem, NewsCache, WarnedResult};
use anyhow::Context;
use arch_updates_rs::{
    check_pacman_updates_online, AurUpdate, AurUpdatesCache, DevelUpdate, DevelUpdatesCache,
    PacmanUpdate, PacmanUpdatesCache,
};
use chrono::{DateTime, Local};
use futures::TryFutureExt;
use std::future::Future;
use tokio::join;

const LOCAL_CHECKUPDATES_LOCK_PATH: &str = "checkupdates.lock";

#[derive(Clone, Copy, Debug)]
pub enum CheckType {
    Online,
    Offline,
}

#[derive(Clone)]
pub struct OnlineNewsResidual {
    pub cache: NewsCache,
    pub time: DateTime<Local>,
}

#[derive(Default, Clone)]
// #[cfg_attr(feature = "mock-api", allow(dead_code))]
pub struct CacheState {
    pacman_cache: Option<PacmanUpdatesCache>,
    aur_cache: Option<AurUpdatesCache>,
    devel_cache: Option<DevelUpdatesCache>,
}

#[derive(Clone, Debug)]
pub struct OnlineUpdates {
    pub pacman: Result<Vec<PacmanUpdate>, String>,
    pub aur: Result<Vec<AurUpdate>, String>,
    pub devel: Result<Vec<DevelUpdate>, String>,
}

#[derive(Clone, Debug)]
// If offline cache didn't exist, it's not an error.
pub struct OfflineUpdates {
    pub pacman: Option<Result<Vec<PacmanUpdate>, String>>,
    pub aur: Option<Result<Vec<AurUpdate>, String>>,
    pub devel: Option<Result<Vec<DevelUpdate>, String>>,
}

/// Shortcut for Vec<T,E> where previous state can be remembered as variant
/// `ErrorWithHistory`
#[derive(Clone, Debug)]
pub enum BasicResultWithHistory<T> {
    Ok { value: T },
    Error,
    ErrorWithHistory { last_value: T },
}

impl<T> BasicResultWithHistory<Vec<T>> {
    /// Returns length of the vector if it's in OK state or has history,
    /// otherwise 0.
    pub fn len(&self) -> usize {
        if let BasicResultWithHistory::Ok { value }
        | BasicResultWithHistory::ErrorWithHistory {
            last_value: value, ..
        } = self
        {
            value.len()
        } else {
            0
        }
    }
}

impl<T> BasicResultWithHistory<T> {
    pub fn has_error(&self) -> bool {
        matches!(self, Self::Error { .. } | Self::ErrorWithHistory { .. })
    }
    /// In the conversion process from a result, error information is lost but
    /// logged to console.
    pub fn new_from_result<E: std::fmt::Display>(value: Result<T, E>) -> Self {
        match value {
            Ok(value) => BasicResultWithHistory::Ok { value },
            Err(e) => {
                eprintln!("{e}");
                BasicResultWithHistory::Error
            }
        }
    }
    /// In the conversion process from a result, error information is lost but
    /// logged to console.
    pub fn replace_with_result_preserving_history<E: std::fmt::Display>(
        self,
        value: Result<T, E>,
    ) -> Self {
        match self {
            BasicResultWithHistory::Ok { value: last_value } => match value {
                Ok(value) => BasicResultWithHistory::Ok { value },
                Err(e) => {
                    eprintln!("{e}");
                    BasicResultWithHistory::ErrorWithHistory { last_value }
                }
            },
            BasicResultWithHistory::Error => match value {
                Ok(value) => BasicResultWithHistory::Ok { value },
                Err(e) => {
                    eprintln!("{e}");
                    BasicResultWithHistory::Error
                }
            },
            BasicResultWithHistory::ErrorWithHistory { last_value, .. } => match value {
                Ok(value) => BasicResultWithHistory::Ok { value },
                Err(e) => {
                    eprintln!("{e}");
                    BasicResultWithHistory::ErrorWithHistory { last_value }
                }
            },
        }
    }
    /// In the conversion process from a result, error information is lost but
    /// logged to console.
    pub fn replace_with_option_result_preserving_history<E: std::fmt::Display>(
        self,
        value: Option<Result<T, E>>,
    ) -> Self {
        let Some(value) = value else { return self };
        match self {
            BasicResultWithHistory::Ok { value: last_value } => match value {
                Ok(value) => BasicResultWithHistory::Ok { value },
                Err(e) => {
                    eprintln!("{e}");
                    BasicResultWithHistory::ErrorWithHistory { last_value }
                }
            },
            BasicResultWithHistory::Error => match value {
                Ok(value) => BasicResultWithHistory::Ok { value },
                Err(e) => {
                    eprintln!("{e}");
                    BasicResultWithHistory::Error
                }
            },
            BasicResultWithHistory::ErrorWithHistory { last_value, .. } => match value {
                Ok(value) => BasicResultWithHistory::Ok { value },
                Err(e) => {
                    eprintln!("{e}");
                    BasicResultWithHistory::ErrorWithHistory { last_value }
                }
            },
        }
    }
}

#[derive(Clone, Debug)]
pub enum TimeoutError<E> {
    Timeout,
    Other(E),
}

impl<E: std::fmt::Display> std::fmt::Display for TimeoutError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TimeoutError::Timeout => write!(f, "Timeout occurred"),
            TimeoutError::Other(e) => write!(f, "{e}"),
        }
    }
}

/// Helper function - adds a timeout to a future that returns a result.
pub async fn flat_timeout<T, E, Fut>(
    duration: std::time::Duration,
    f: Fut,
) -> Result<T, TimeoutError<E>>
where
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let res = tokio::time::timeout(duration, f)
        .map_err(|_| TimeoutError::Timeout)
        .await;
    match res {
        Ok(Err(e)) => Err(TimeoutError::Other(e)),
        Err(e) => Err(e),
        Ok(Ok(t)) => Ok(t),
    }
}

/// Turn a WarnedResult into a Result, emitting an effect if a warning existed
/// (print to stderr).
pub fn consume_warning<T, W: std::fmt::Display, E>(w: WarnedResult<T, W, E>) -> Result<T, E> {
    match w {
        WarnedResult::Ok(t) => Ok(t),
        WarnedResult::Warning(t, w) => {
            eprintln!("Warning: {w}");
            Ok(t)
        }
        WarnedResult::Err(e) => Err(e),
    }
}

#[cfg_attr(feature = "mock-api", allow(unused_variables, unreachable_code))]
pub async fn get_news_offline(
    cache: &NewsCache,
) -> WarnedResult<Vec<DatedNewsItem>, String, anyhow::Error> {
    #[cfg(feature = "mock-api")]
    return super::mock::get_mock_news().await;

    crate::news::get_news_offline(cache).await
}

#[cfg_attr(feature = "mock-api", allow(unreachable_code))]
pub async fn get_news_online(
) -> WarnedResult<(Vec<DatedNewsItem>, NewsCache), String, anyhow::Error> {
    #[cfg(feature = "mock-api")]
    return super::mock::get_mock_news()
        .await
        .map(|r| (r, NewsCache::default()));

    crate::news::get_news_online().await
}

#[cfg_attr(feature = "mock-api", allow(unused_variables, unreachable_code))]
pub async fn get_updates_offline(cache: &CacheState) -> OfflineUpdates {
    #[cfg(feature = "mock-api")]
    return super::mock::get_mock_updates().await;

    let CacheState {
        aur_cache,
        devel_cache,
        pacman_cache,
    } = cache;
    async fn async_map<T, U>(t: &Option<T>, f: impl AsyncFn(&T) -> U) -> Option<U> {
        match t {
            Some(t) => Some(f(t).await),
            None => None,
        }
    }
    let (pacman, aur, devel) = join!(
        async_map(pacman_cache, arch_updates_rs::check_pacman_updates_offline),
        async_map(aur_cache, arch_updates_rs::check_aur_updates_offline),
        async_map(devel_cache, arch_updates_rs::check_devel_updates_offline),
    );
    // String conversion of errors required as arch_updates_rs::Error is not Clone.
    let pacman = pacman.map(|r| r.map_err(|e| format!("ERROR - pacman updates: {e}")));
    let aur = aur.map(|r| r.map_err(|e| format!("ERROR - AUR updates: {e}")));
    let devel = devel.map(|r| r.map_err(|e| format!("ERROR - devel updates: {e}")));
    OfflineUpdates { pacman, aur, devel }
}

/// [[arch_updates_rs::check_pacman_updates_online]] can't run concurrently, so
/// this is a wrapper around it that uses a file lock to ensure only one
/// `cosmic-applet-arch` process is running it.
/// # Notes
/// 1. This will still error if someone else's process is running
///    `checkupdates`! Since the app continuously polls for updates this should
///    have a small impact only.
/// 2. Recommend running this under a timeout incase lock somehow deadlocks.
pub async fn check_pacman_updates_online_exclusive(
) -> anyhow::Result<(Vec<PacmanUpdate>, PacmanUpdatesCache)> {
    let proj_dirs = proj_dirs().context("Unable to obtain a local data storage directory")?;
    tokio::fs::create_dir_all(proj_dirs.data_local_dir())
        .await
        .context("Unable to create local data storage directory")?;
    let lock_file_path = proj_dirs
        .data_local_dir()
        .to_path_buf()
        .join(LOCAL_CHECKUPDATES_LOCK_PATH);
    let _guard = crate::app::async_file_lock::AsyncFileLock::new(lock_file_path)
        .await
        .context("Unable to obtain a lock for use of checkupdates")?;
    Ok(check_pacman_updates_online().await?)
}

#[cfg_attr(feature = "mock-api", allow(unused_variables, unreachable_code))]
pub async fn get_updates_online(timeout: std::time::Duration) -> (OnlineUpdates, CacheState) {
    #[cfg(feature = "mock-api")]
    return (
        OnlineUpdates {
            pacman: Ok(Default::default()),
            aur: Ok(Default::default()),
            devel: Ok(Default::default()),
        },
        CacheState {
            pacman_cache: None,
            aur_cache: None,
            devel_cache: None,
        },
    );

    let (pacman, aur, devel) = join!(
        // arch_updates_rs::check_pacman_updates_online doesn't handle multiple concurrent
        // processes.
        flat_timeout(timeout, check_pacman_updates_online_exclusive()),
        flat_timeout(timeout, arch_updates_rs::check_aur_updates_online()),
        flat_timeout(timeout, arch_updates_rs::check_devel_updates_online()),
    );
    fn extract_cache_and_update<U, C, E>(update: Result<(U, C), E>) -> (Option<C>, Result<U, E>) {
        match update {
            Ok((update, cache)) => (Some(cache), Ok(update)),
            Err(e) => (None, Err(e)),
        }
    }
    let (pacman_cache, pacman_updates) = extract_cache_and_update(pacman);
    let (aur_cache, aur_updates) = extract_cache_and_update(aur);
    let (devel_cache, devel_updates) = extract_cache_and_update(devel);
    // String conversion of errors required as arch_updates_rs::Error is not Clone.
    let updates = OnlineUpdates {
        pacman: pacman_updates.map_err(|e| format!("ERROR - pacman updates: {e}")),
        aur: aur_updates.map_err(|e| format!("ERROR - AUR updates: {e}")),
        devel: devel_updates.map_err(|e| format!("ERROR - devel updates: {e}")),
    };
    (
        updates,
        CacheState {
            pacman_cache,
            aur_cache,
            devel_cache,
        },
    )
}

#[cfg(test)]
mod tests {
    use crate::app::subscription::core::check_pacman_updates_online_exclusive;
    use futures::future::try_join;

    #[tokio::test]
    #[ignore = "Effectful test (local storage)"]
    async fn test_concurrent_check_pacman_updates_online_exclusive() {
        // Running this function concurrently should not cause errors.
        try_join(
            check_pacman_updates_online_exclusive(),
            check_pacman_updates_online_exclusive(),
        )
        .await
        .unwrap();
    }
}
