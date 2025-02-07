//! API for feature to fetch latest news from arch RSS feed.
use cosmic::cctk::wayland_protocols::wp::input_timestamps::zv1::client::zwp_input_timestamps_manager_v1::REQ_GET_KEYBOARD_TIMESTAMPS_OPCODE;
use error::*;
use rss::Channel;
use std::{io::BufReader, str::FromStr};

/// To avoid displaying all news, we need to know when the system was last
/// updated. Only show news later than that.
mod latest_update {}

mod error {
    #[derive(Debug)]
    pub enum NewsError {
        Web(reqwest::Error),
        Rss(rss::Error),
    }

    impl From<rss::Error> for NewsError {
        fn from(e: rss::Error) -> NewsError {
            NewsError::Rss(e)
        }
    }
    impl From<reqwest::Error> for NewsError {
        fn from(e: reqwest::Error) -> NewsError {
            NewsError::Web(e)
        }
    }
}

struct NewsItem<Tz>
where
    Tz: chrono::TimeZone,
{
    title: Option<String>,
    link: Option<String>,
    description: Option<String>,
    author: Option<String>,
    date: chrono::DateTime<Tz>,
}

impl<Tz: chrono::TimeZone> NewsItem<Tz> {
    fn from_source(item: rss::Item) -> Option<NewsItem<chrono::FixedOffset>> {
        let rss::Item {
            title,
            link,
            description,
            author,
            pub_date,
            ..
        } = item;
        let date = pub_date?;
        let date = chrono::DateTime::parse_from_rfc2822(&date).unwrap();
        Some(NewsItem {
            title,
            link,
            description,
            author,
            date,
        })
    }
}

async fn get_latest_arch_news(
    last_updated_dt: std::time::SystemTime,
) -> Result<Vec<NewsItem<chrono::FixedOffset>>, NewsError> {
    let feed = get_arch_rss_feed().await?;
    feed.items.into_iter().filter(predicate);
}

async fn get_arch_rss_feed() -> Result<Channel, NewsError> {
    let content = reqwest::get("https://archlinux.org/feeds/news/")
        .await?
        .bytes()
        .await?;
    let channel = Channel::read_from(&content[..])?;
    Ok(channel)
}

// TODO: Relegate to mock feature.
/// May panic
async fn get_mock_rss_feed() -> Channel {
    let file = tokio::fs::File::open("test/mock_feed.rss").await.unwrap();
    Channel::read_from(BufReader::new(file.into_std().await)).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[ignore = "Ignore by default as this uses the network"]
    #[tokio::test]
    async fn arch_rss_feed_is_ok() {
        let feed = get_arch_rss_feed().await;
        assert!(feed.is_ok())
    }

    #[tokio::test]
    async fn mock_rss_feed_is_ok() {
        // May panic, that's the fail case for this test.
        get_mock_rss_feed().await;
    }
}
