//! API for feature to fetch latest news from arch RSS feed.
use anyhow::Result;
use chrono::FixedOffset;
use latest_update::Arch;
pub use news_impl::DatedNewsItem;
use news_impl::{get_latest_arch_news, Network};

mod latest_update;
mod news_impl;

#[derive(Clone)]
#[cfg_attr(feature = "mock-api", allow(dead_code))]
pub struct NewsCache(Vec<DatedNewsItem>);

pub async fn get_news_online(
) -> WarnedResult<(Vec<DatedNewsItem>, NewsCache), String, anyhow::Error> {
    latest_update::get_latest_update(&Arch)
        .await
        .async_and_then(async |cutoff| get_latest_arch_news(&Network, cutoff).await)
        .await
        .map(|updates| (updates.clone(), NewsCache(updates)))
}

#[cfg_attr(feature = "mock-api", allow(dead_code))]
pub async fn get_news_offline(
    cache: &NewsCache,
) -> WarnedResult<Vec<DatedNewsItem>, String, anyhow::Error> {
    latest_update::get_latest_update(&Arch).await.map(|cutoff| {
        NewsCache::clone(cache)
            .0
            .into_iter()
            .filter(|item| cutoff.is_none_or(|cutoff| item.date >= cutoff))
            .collect()
    })
}

pub async fn set_news_last_read(dt: chrono::DateTime<FixedOffset>) -> Result<()> {
    latest_update::set_local_last_read(&Arch, dt).await
}

/// Represents a Result with a 3rd state, Warning, that allows you to access the
/// inner value but also a warning for it.
pub enum WarnedResult<T, W, E> {
    Ok(T),
    Warning(T, W),
    Err(E),
}

impl<T, W, E> WarnedResult<T, W, E> {
    pub fn from_result(r: Result<T, E>) -> Self {
        match r {
            Ok(t) => WarnedResult::Ok(t),
            Err(e) => WarnedResult::Err(e),
        }
    }
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> WarnedResult<U, W, E> {
        match self {
            WarnedResult::Ok(t) => WarnedResult::Ok(f(t)),
            WarnedResult::Warning(t, w) => WarnedResult::Warning(f(t), w),
            WarnedResult::Err(e) => WarnedResult::Err(e),
        }
    }
    pub async fn async_and_then<U>(
        self,
        f: impl AsyncFnOnce(T) -> Result<U, E>,
    ) -> WarnedResult<U, W, E> {
        match self {
            WarnedResult::Ok(t) => match f(t).await {
                Ok(u) => WarnedResult::Ok(u),
                Err(e) => WarnedResult::Err(e),
            },
            WarnedResult::Warning(t, w) => match f(t).await {
                Ok(u) => WarnedResult::Warning(u, w),
                Err(e) => WarnedResult::Err(e),
            },
            WarnedResult::Err(e) => WarnedResult::Err(e),
        }
    }
    #[cfg(feature = "mock-api")]
    pub fn is_ok(&self) -> bool {
        matches!(self, WarnedResult::Ok(_))
    }
}
