//! API for feature to fetch latest news from arch RSS feed.
use rss::Channel;

/// To avoid displaying all news, we need to know when the system was last
/// updated. Only show news later than that.
mod latest_update {}

enum NewsError {
    Web(reqwest::Error),
    Rss(rss::Error),
}

impl From<rss::Error> for NewsError {
    fn from(self) -> NewsError {
        NewsError::Rss(self)
    }
}
impl From<reqwest::Error> for NewsError {
    fn from(self) -> NewsError {
        NewsError::Web(self)
    }
}

async fn get_arch_rss_feed() -> Result<Channel, NewsError> {
    let content = reqwest::get("https://archlinux.org/feeds/news/")
        .await?
        .bytes()
        .await?;
    let channel = Channel::read_from(&content[..])?;
    Ok(channel)
}

#[cfg(test)]
mod tests {}
