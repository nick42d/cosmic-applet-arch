use crate::news::NewsCache;
use crate::news::{DatedNewsItem, WarnedResult};
use arch_updates_rs::{
    AurUpdate, AurUpdatesCache, DevelUpdate, DevelUpdatesCache, PacmanUpdate, PacmanUpdatesCache,
};
use chrono::{DateTime, Local};
use futures::TryFutureExt;
use std::future::Future;
use tokio::join;

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
pub struct CacheState {
    pacman_cache: PacmanUpdatesCache,
    aur_cache: AurUpdatesCache,
    devel_cache: DevelUpdatesCache,
}

#[derive(Clone, Debug, Default)]
pub struct Updates {
    pub pacman: Vec<PacmanUpdate>,
    pub aur: Vec<AurUpdate>,
    pub devel: Vec<DevelUpdate>,
}

impl Updates {
    pub fn total(&self) -> usize {
        self.pacman.len() + self.aur.len() + self.devel.len()
    }
}

/// Helper function - adds a timeout to a future that returns a result.
/// Type erases the error by converting to string, avoiding nested results.
pub async fn flat_erased_timeout<T, E, Fut>(
    duration: std::time::Duration,
    f: Fut,
) -> Result<T, String>
where
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let res = tokio::time::timeout(duration, f.map_err(|e| format!("{e}")))
        .map_err(|_| "API call timed out".to_string())
        .await;
    match res {
        Ok(Err(e)) | Err(e) => Err(e),
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
    mock::get_mock_news().await
}

#[cfg(not(feature = "mock-api"))]
pub async fn get_news_offline(
    cache: &NewsCache,
) -> WarnedResult<Vec<DatedNewsItem>, String, anyhow::Error> {
    crate::news::get_news_offline(&cache).await
}

pub async fn get_news_online(
) -> WarnedResult<(Vec<DatedNewsItem>, NewsCache), String, anyhow::Error> {
    crate::news::get_news_online().await
}

#[cfg(feature = "mock-api")]
pub async fn get_updates_offline(_: &CacheState) -> arch_updates_rs::Result<Updates> {
    mock::get_mock_updates().await
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

pub async fn get_updates_online() -> arch_updates_rs::Result<(Updates, CacheState)> {
    let (pacman, aur, devel) = join!(
        arch_updates_rs::check_pacman_updates_online(),
        arch_updates_rs::check_aur_updates_online(),
        arch_updates_rs::check_devel_updates_online(),
    );
    let (pacman, pacman_cache) = pacman?;
    let (aur, aur_cache) = aur?;
    let (devel, devel_cache) = devel?;
    Ok((
        Updates { pacman, aur, devel },
        CacheState {
            aur_cache,
            devel_cache,
            pacman_cache,
        },
    ))
}
