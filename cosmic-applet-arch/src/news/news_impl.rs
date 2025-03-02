use anyhow::{Context, Result};
use chrono::FixedOffset;
use itertools::Itertools;
use rss::Channel;

const ARCH_NEWS_FEED_URL: &str = "https://archlinux.org/feeds/news/";

#[cfg_attr(test, mockall::automock)]
pub trait ArchNewsFeed {
    async fn get_arch_news_feed(&self) -> Result<Channel>;
}

pub struct Network;

impl ArchNewsFeed for Network {
    async fn get_arch_news_feed(&self) -> Result<Channel> {
        let content = reqwest::get(ARCH_NEWS_FEED_URL).await?.bytes().await?;
        let channel = Channel::read_from(&content[..])?;
        Ok(channel)
    }
}

#[derive(PartialEq, Debug, Clone)]
pub struct DatedNewsItem {
    pub title: Option<String>,
    pub link: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub date: chrono::DateTime<FixedOffset>,
}

impl DatedNewsItem {
    fn from_source(item: rss::Item) -> Result<DatedNewsItem> {
        let rss::Item {
            title,
            link,
            description,
            pub_date,
            dublin_core_ext,
            ..
        } = item;
        // Can't do much with an rss feed item without a date, so this is an error.
        let date = pub_date.context("Arch news item missing date")?;
        let date = chrono::DateTime::parse_from_rfc2822(&date).context(format!(
            "Error parsing date from arch rss feed - {:?}",
            &date
        ))?;
        let author = dublin_core_ext.map(|dc| dc.creators.into_iter().join(", "));
        Ok(DatedNewsItem {
            title,
            link,
            description,
            author,
            date,
        })
    }
}

pub async fn get_latest_arch_news(
    feed: &impl ArchNewsFeed,
    last_updated_dt: chrono::DateTime<chrono::FixedOffset>,
) -> Result<Vec<DatedNewsItem>> {
    let feed = feed
        .get_arch_news_feed()
        .await
        .context("Error getting arch news feed")?;
    feed.items
        .into_iter()
        .map(DatedNewsItem::from_source)
        .process_results(|iter| iter.filter(|item| item.date >= last_updated_dt).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::io::BufReader;

    /// May panic
    async fn get_mock() -> impl ArchNewsFeed {
        let file = tokio::fs::File::open("test/mock_feed.rss").await.unwrap();
        let feed = Channel::read_from(BufReader::new(file.into_std().await)).unwrap();
        let mut mock = MockArchNewsFeed::new();
        mock.expect_get_arch_news_feed()
            .return_once(move || Ok(feed));
        mock
    }

    #[ignore = "Effectful test (network)"]
    #[tokio::test]
    async fn arch_rss_feed_is_ok() {
        let platform = Network;
        let feed = platform.get_arch_news_feed().await;
        assert!(feed.is_ok())
    }
    #[tokio::test]
    async fn test_get_latest_news_local_vs_utc() {
        let mock = get_mock().await;
        let latest_news = get_latest_arch_news(
            &mock,
            chrono::FixedOffset::east_opt(8 * 60 * 60)
                .unwrap()
                .with_ymd_and_hms(2025, 1, 16, 15, 33, 43)
                .unwrap(),
        )
        .await
        .unwrap();
        // Need to whiteboard an figure out why failing.
        assert_eq!(latest_news.len(), 1);
    }
    #[tokio::test]
    async fn test_get_latest_news_multiple() {
        let mock = get_mock().await;
        let latest_news = get_latest_arch_news(
            &mock,
            chrono::Utc
                .with_ymd_and_hms(2024, 11, 20, 0, 0, 0)
                .unwrap()
                .into(),
        )
        .await
        .unwrap();
        assert_eq!(latest_news.len(), 2);
    }
    #[tokio::test]
    async fn test_get_latest_news_one() {
        let mock = get_mock().await;
        let latest_news = get_latest_arch_news(
            &mock,
            chrono::Utc
                .with_ymd_and_hms(2025, 2, 2, 0, 0, 0)
                .unwrap()
                .into(),
        )
        .await
        .unwrap();
        assert_eq!(latest_news.len(), 1);
    }
    #[tokio::test]
    async fn test_feed_has_all_items() {
        // May panic, that's the fail case for this test.
        let feed = get_mock().await.get_arch_news_feed().await.unwrap();
        let items = feed
            .items
            .into_iter()
            .map(DatedNewsItem::from_source)
            .collect::<Result<Vec<_>>>()
            .unwrap();
        assert!(items.len() == 10);
    }

    #[tokio::test]
    async fn test_feed_has_specific_item() {
        // May panic, that's the fail case for this test.
        let feed = get_mock().await.get_arch_news_feed().await.unwrap();
        let item_one = feed
            .items
            .into_iter()
            .map(DatedNewsItem::from_source)
            .next()
            .unwrap()
            .unwrap();
        let expected_dt = chrono::Utc
            .with_ymd_and_hms(2025, 2, 3, 11, 24, 25)
            .unwrap();
        let expected = DatedNewsItem {
            title: Some("Glibc 2.41 corrupting Discord installation".to_string()),
            link: Some(
                "https://archlinux.org/news/glibc-241-corrupting-discord-installation/".to_string(),
            ),
            description: Some("<p>We plan to move <code>glibc</code> and its friends to stable later today, Feb 3. After installing the update, the Discord client will show a red warning that the installation is corrupt.</p>\n<p>This issue has been fixed in the Discord canary build. If you rely on audio connectivity, please use the canary build, login via browser or the flatpak version until the fix hits the stable Discord release.</p>\n<p>There have been no reports that (written) chat connectivity is affected.</p>\n<p>UPDATE: The issue has been fixed in Discord <code>0.0.84-1</code>.</p>".to_string()),
            author: Some("Frederik Schwan".to_string()),
            date: expected_dt.into(),
        };
        assert_eq!(expected, item_one);
    }
}
