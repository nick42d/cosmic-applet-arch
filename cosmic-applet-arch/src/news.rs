//! API for feature to fetch latest news from arch RSS feed.
use std::io::BufReader;

use error::*;
use rss::Channel;

/// To avoid displaying all news, we need to know when the system was last
/// updated. Only show news later than that.
mod latest_update {
    const PACMAN_LOG_PATH: &str = "/var/log/pacman.log";

    async fn get_pacman_log() -> Result<String, std::io::Error> {
        #[cfg(feature = "mock-api")]
        {
            return Ok(include_str!("../test/pacman.log").to_string());
        }

        tokio::fs::read_to_string(PACMAN_LOG_PATH).await
    }

    pub async fn get_latest_update() -> Result<chrono::NaiveDateTime, std::io::Error> {
        let log = get_pacman_log().await?;
        let last_update_line = log
            .lines()
            .filter(|line| line.contains("starting full system upgrade"))
            .last()
            .unwrap();
        let last_update_str = last_update_line
            .trim_start_matches('[')
            .split(']')
            .next()
            .unwrap();
        Ok(chrono::NaiveDateTime::parse_from_str(last_update_str, "").unwrap())
    }
}

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

struct DatedNewsItem<Tz>
where
    Tz: chrono::TimeZone,
{
    title: Option<String>,
    link: Option<String>,
    description: Option<String>,
    author: Option<String>,
    date: chrono::DateTime<Tz>,
}

impl DatedNewsItem<chrono::FixedOffset> {
    fn from_source(item: rss::Item) -> Option<DatedNewsItem<chrono::FixedOffset>> {
        let rss::Item {
            title,
            link,
            description,
            author,
            pub_date,
            ..
        } = item;
        // Should not having a date be an error?
        let date = pub_date?;
        let date = chrono::DateTime::parse_from_rfc2822(&date).ok()?;
        Some(DatedNewsItem {
            title,
            link,
            description,
            author,
            date,
        })
    }
}

async fn get_latest_arch_news(
    last_updated_dt: chrono::DateTime<chrono::FixedOffset>,
) -> Result<Vec<DatedNewsItem<chrono::FixedOffset>>, NewsError> {
    let feed = get_arch_rss_feed().await?;
    Ok(feed
        .items
        .into_iter()
        .filter_map(DatedNewsItem::from_source)
        .filter(|item| item.date < last_updated_dt)
        .collect())
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

    #[ignore = "Effectful test (network)"]
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

    #[tokio::test]
    #[cfg(feature = "mock-api")]
    async fn mock_get_latest_update() {
        use latest_update::get_latest_update;

        assert_eq!(
            get_latest_update().await.unwrap(),
            chrono::NaiveDateTime::UNIX_EPOCH
        )
    }
}
