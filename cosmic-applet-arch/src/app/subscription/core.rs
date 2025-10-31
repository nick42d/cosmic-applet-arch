use crate::core::config::UpdateType;
use crate::core::proj_dirs;
use crate::news::{DatedNewsItem, NewsCache, WarnedResult};
use anyhow::Context;
use arch_updates_rs::{
    check_pacman_updates_online, AurUpdate, AurUpdatesCache, DevelUpdate, DevelUpdatesCache,
    PacmanUpdate, PacmanUpdatesCache,
};
use chrono::{DateTime, Local};
use cosmic::cosmic_config::Update;
use futures::TryFutureExt;
use std::collections::HashSet;
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
pub struct OnlineUpdateResidual {
    pub cache: CacheState,
    pub time: DateTime<Local>,
}

#[derive(Default, Clone)]
#[cfg_attr(feature = "mock-api", allow(dead_code))]
pub struct CacheState {
    pacman_cache: PacmanUpdatesCache,
    aur_cache: AurUpdatesCache,
    devel_cache: DevelUpdatesCache,
}

#[derive(Debug, Clone)]
pub struct UpdatesError;

#[derive(Clone, Debug)]
pub struct OnlineUpdatesMessage {
    pub pacman: Result<Vec<PacmanUpdate>, UpdatesError>,
    pub aur: Result<Vec<AurUpdate>, UpdatesError>,
    pub devel: Result<Vec<DevelUpdate>, UpdatesError>,
    pub update_time: chrono::DateTime<Local>,
}

#[derive(Clone, Debug)]
// If offline cache didn't exist, it's not an error.
pub struct OfflineUpdatesMessage {
    pub pacman: Option<Result<Vec<PacmanUpdate>, UpdatesError>>,
    pub aur: Option<Result<Vec<AurUpdate>, UpdatesError>>,
    pub devel: Option<Result<Vec<DevelUpdate>, UpdatesError>>,
}

/// Shortcut for Vec<T,E> where previous state can be remembered as variant
/// `ErrorWithHistory`
#[derive(Clone, Debug)]
pub enum ErrorVecWithHistory<T, E> {
    Ok { value: Vec<T> },
    Error { error: E },
    ErrorWithHistory { last_value: Vec<T>, error: E },
}

impl<T, E> ErrorVecWithHistory<T, E> {
    /// Returns length of the vector if it's in OK state, otherwise 0.
    pub fn len(&self) -> usize {
        if let ErrorVecWithHistory::Ok { value } = self {
            value.len()
        } else {
            0
        }
    }
    pub fn new_from_result(value: Result<Vec<T>, E>) -> Self {
        match value {
            Ok(value) => ErrorVecWithHistory::Ok { value },
            Err(error) => ErrorVecWithHistory::Error { error },
        }
    }
    pub fn replace_with_result_preserving_history(self, value: Result<Vec<T>, E>) -> Self {
        match self {
            ErrorVecWithHistory::Ok { value: last_value } => match value {
                Ok(value) => ErrorVecWithHistory::Ok { value },
                Err(error) => ErrorVecWithHistory::ErrorWithHistory { last_value, error },
            },
            ErrorVecWithHistory::Error { error } => match value {
                Ok(value) => ErrorVecWithHistory::Ok { value },
                Err(error) => ErrorVecWithHistory::Error { error },
            },
            ErrorVecWithHistory::ErrorWithHistory { last_value, error } => match value {
                Ok(value) => ErrorVecWithHistory::Ok { value },
                Err(error) => ErrorVecWithHistory::ErrorWithHistory { last_value, error },
            },
        }
    }
    pub fn replace_with_option_result_preserving_history(
        self,
        value: Option<Result<Vec<T>, E>>,
    ) -> Self {
        let Some(value) = value else { return self };
        match self {
            ErrorVecWithHistory::Ok { value: last_value } => match value {
                Ok(value) => ErrorVecWithHistory::Ok { value },
                Err(error) => ErrorVecWithHistory::ErrorWithHistory { last_value, error },
            },
            ErrorVecWithHistory::Error { .. } => match value {
                Ok(value) => ErrorVecWithHistory::Ok { value },
                Err(error) => ErrorVecWithHistory::Error { error },
            },
            ErrorVecWithHistory::ErrorWithHistory { last_value, .. } => match value {
                Ok(value) => ErrorVecWithHistory::Ok { value },
                Err(error) => ErrorVecWithHistory::ErrorWithHistory { last_value, error },
            },
        }
    }
}

impl<T, E> Default for ErrorVecWithHistory<T, E> {
    fn default() -> Self {
        Self::Ok { value: vec![] }
    }
}

#[derive(Clone, Debug)]
enum TimeoutError<E> {
    Timeout,
    Other(E),
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

#[cfg(feature = "mock-api")]
pub async fn get_news_offline(
    _: &NewsCache,
) -> WarnedResult<Vec<DatedNewsItem>, String, anyhow::Error> {
    super::mock::get_mock_news().await
}

#[cfg(not(feature = "mock-api"))]
pub async fn get_news_offline(
    cache: &NewsCache,
) -> WarnedResult<Vec<DatedNewsItem>, String, anyhow::Error> {
    crate::news::get_news_offline(cache).await
}

pub async fn get_news_online(
) -> WarnedResult<(Vec<DatedNewsItem>, NewsCache), String, anyhow::Error> {
    crate::news::get_news_online().await
}

#[cfg(feature = "mock-api")]
pub async fn get_updates_offline(_: &CacheState) -> arch_updates_rs::Result<Updates> {
    super::mock::get_mock_updates().await
}

#[cfg(not(feature = "mock-api"))]
pub async fn get_updates_offline(cache: &CacheState) -> arch_updates_rs::Result<Updates> {
    let CacheState {
        aur_cache,
        devel_cache,
        pacman_cache,
    } = cache;
    let (pacman, aur, devel) = join!(
        arch_updates_rs::check_pacman_updates_offline(pacman_cache),
        arch_updates_rs::check_aur_updates_offline(aur_cache),
        arch_updates_rs::check_devel_updates_offline(devel_cache),
    );
    Ok(Updates {
        pacman: pacman?,
        aur: aur?,
        devel: devel?,
    })
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

// pub async fn get_updates_online(prev_val: Updates) ->
// anyhow::Result<(Updates, CacheState)> {     let (pacman, aur, devel) = join!(
//         // arch_updates_rs::check_pacman_updates_online doesn't handle
// multiple concurrent         // processes.
//         check_pacman_updates_online_exclusive(),
//         arch_updates_rs::check_aur_updates_online(),
//         arch_updates_rs::check_devel_updates_online(),
//     );
//     let (pacman, pacman_cache) = pacman?;
//     let (aur, aur_cache) = aur?;
//     let (devel, devel_cache) = devel?;
//     Ok((
//         Updates { pacman, aur, devel },
//         CacheState {
//             aur_cache,
//             devel_cache,
//             pacman_cache,
//         },
//     ))
// }

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
