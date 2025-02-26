//! API for feature to fetch latest news from arch RSS feed.
use anyhow::Result;
use chrono::FixedOffset;
use latest_update::Arch;
use news_impl::{get_latest_arch_news, Network};

pub use news_impl::DatedNewsItem;

mod latest_update;
mod news_impl;

pub async fn get_news_online() -> WarnedResult<Vec<DatedNewsItem>, String, anyhow::Error> {
    latest_update::get_latest_update(&Arch)
        .await
        .async_and_then(async |cutoff| get_latest_arch_news(&Network, cutoff.into()).await)
        .await
}

pub async fn get_news_offline(
    cache: Vec<DatedNewsItem>,
) -> WarnedResult<Vec<DatedNewsItem>, String, anyhow::Error> {
    latest_update::get_latest_update(&Arch).await.map(|cutoff| {
        cache
            .into_iter()
            .filter(|item| item.date >= cutoff)
            .collect()
    })
}

pub async fn set_news_last_read(dt: chrono::DateTime<FixedOffset>) -> Result<()> {
    latest_update::set_local_last_read(&Arch, dt).await
}

/// Represents a Result with a 3rd state, Warning, that allows you to access the inner value but also a warning for it.
enum WarnedResult<T, W, E> {
    Ok(T),
    Warning(T, W),
    Err(E),
}

impl<T, W, E> WarnedResult<T, W, E> {
    fn map<U>(self, f: impl FnOnce(T) -> U) -> WarnedResult<U, W, E> {
        match self {
            WarnedResult::Ok(t) => WarnedResult::Ok(f(t)),
            WarnedResult::Warning(t, w) => WarnedResult::Warning(f(t), w),
            WarnedResult::Err(e) => WarnedResult::Err(e),
        }
    }
    async fn async_and_then<U>(
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
}
