//! API for feature to fetch latest news from arch RSS feed.
use chrono::FixedOffset;
use error::*;
use rss::Channel;

/// To avoid displaying all news, we need to know when the system was last
/// updated. Only show news later than that.
mod latest_update {
    use anyhow::{anyhow, bail, Context};
    use chrono::{DateTime, Local};
    use directories::ProjectDirs;

    const PACMAN_LOG_PATH: &str = "/var/log/pacman.log";
    const LOCAL_LAST_READ_PATH: &str = "last_read";

    #[cfg_attr(test, mockall::automock)]
    trait ArchInstallation {
        async fn get_pacman_log(&self) -> std::io::Result<String>;
        async fn get_local_update_timestamp_file(&self) -> std::io::Result<String>;
    }

    struct Arch;
    impl ArchInstallation for Arch {
        async fn get_pacman_log(&self) -> std::io::Result<String> {
            tokio::fs::read_to_string(PACMAN_LOG_PATH).await
        }
        async fn get_local_update_timestamp_file(&self) -> std::io::Result<String> {
            let proj_dirs = ProjectDirs::from("com", "nick42d", "cosmic-applet-arch").unwrap();
            tokio::fs::read_to_string(
                proj_dirs
                    .data_local_dir()
                    .to_path_buf()
                    .join(LOCAL_LAST_READ_PATH),
            )
            .await
        }
    }
    pub async fn get_latest_update(
        platform: &impl ArchInstallation,
    ) -> anyhow::Result<DateTime<Local>> {
        let (local_last_read, latest_pacman_update) = futures::future::join(
            get_local_last_read(platform),
            get_latest_pacman_update(platform),
        )
        .await;
        match (local_last_read, latest_pacman_update) {
            (Ok(local_dt), Ok(pacman_dt)) => Ok(local_dt.max(pacman_dt)),
            (Ok(dt), Err(e)) | (Err(e), Ok(dt)) => {
                eprintln!("Recieved an error determining last update, but there was a fallback method that could be used {e}");
                Ok(dt)
            }
            (Err(e1), Err(e2)) => bail!("Errors determining last update, {e1}, {e2}"),
        }
    }
    pub async fn set_local_last_read(datetime: DateTime<Local>) -> anyhow::Result<()> {
        let proj_dirs = ProjectDirs::from("com", "nick42d", "cosmic-applet-arch")
            .context("Error determining local data directory")?;
        tokio::fs::write(
            proj_dirs
                .data_local_dir()
                .to_path_buf()
                .join(LOCAL_LAST_READ_PATH),
            datetime.to_rfc3339(),
        )
        .await
        .context("Error writing last read to disk")
    }
    async fn get_local_last_read(
        platform: &impl ArchInstallation,
    ) -> anyhow::Result<DateTime<Local>> {
        let last_read_string = platform
            .get_local_update_timestamp_file()
            .await
            .context("Error reading last read file")?;
        let date_time = DateTime::parse_from_rfc3339(&last_read_string)
            .context("Error parsing last read file")?;
        Ok(DateTime::<Local>::from(date_time))
    }
    async fn get_latest_pacman_update(
        platform: &impl ArchInstallation,
    ) -> anyhow::Result<DateTime<Local>> {
        let log = platform
            .get_pacman_log()
            .await
            .context("Error reading pacman log")?;
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
        let naive_datetime = DateTime::parse_from_str(last_update_str, "")
            .context("Error parsing pacman log timestamp")?;
        Ok(DateTime::<Local>::from(naive_datetime))
    }
    #[cfg(test)]
    mod tests {
        use crate::news::latest_update::{
            get_latest_pacman_update, get_local_last_read, MockArchInstallation,
        };

        #[tokio::test]
        async fn mock_get_latest_pacman_update() {
            let mut mock = MockArchInstallation::new();
            mock.expect_get_pacman_log()
                .returning(|| Ok(include_str!("../test/pacman.log").to_string()));
            assert_eq!(
                get_latest_pacman_update(&mock).await.unwrap(),
                chrono::DateTime::<chrono::Local>::from(chrono::DateTime::UNIX_EPOCH)
            );
        }
        #[tokio::test]
        async fn mock_get_latest_local_update() {
            let mut mock = MockArchInstallation::new();
            mock.expect_get_local_update_timestamp_file()
                .returning(|| Ok("".to_string()));
            assert_eq!(
                get_local_last_read(&mock).await.unwrap(),
                chrono::DateTime::<chrono::Local>::from(chrono::DateTime::UNIX_EPOCH)
            );
        }
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

#[cfg_attr(test, mockall::automock)]
trait ArchNewsFeed {
    async fn get_feed(&self) -> Result<Channel, NewsError>;
}

struct Network;

const ARCH_NEWS_FEED_URL: &str = "https://archlinux.org/feeds/news/";

impl ArchNewsFeed for Network {
    async fn get_feed(&self) -> Result<Channel, NewsError> {
        let content = reqwest::get("https://archlinux.org/feeds/news/")
            .await?
            .bytes()
            .await?;
        let channel = Channel::read_from(&content[..])?;
        Ok(channel)
    }
}

#[derive(PartialEq, Debug)]
struct DatedNewsItem {
    title: Option<String>,
    link: Option<String>,
    description: Option<String>,
    author: Option<String>,
    date: chrono::DateTime<FixedOffset>,
}

impl DatedNewsItem {
    fn from_source(item: rss::Item) -> Option<DatedNewsItem> {
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
) -> Result<Vec<DatedNewsItem>, NewsError> {
    let feed = get_arch_rss_feed().await?;
    Ok(feed
        .items
        .into_iter()
        .filter_map(DatedNewsItem::from_source)
        .filter(|item| item.date < last_updated_dt)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufReader;

    /// May panic
    async fn get_mock_rss_feed() -> Channel {
        let file = tokio::fs::File::open("test/mock_feed.rss").await.unwrap();
        Channel::read_from(BufReader::new(file.into_std().await)).unwrap()
    }

    #[ignore = "Effectful test (network)"]
    #[tokio::test]
    async fn arch_rss_feed_is_ok() {
        let feed = get_arch_rss_feed().await;
        assert!(feed.is_ok())
    }

    #[tokio::test]
    async fn mock_rss_feed_has_all_items() {
        // May panic, that's the fail case for this test.
        let feed = get_mock_rss_feed().await;
        let items: Vec<_> = feed
            .items
            .into_iter()
            .filter_map(DatedNewsItem::from_source)
            .collect();
        assert!(items.len() == 10);
    }

    #[tokio::test]
    async fn mock_rss_feed_has_specific_item() {
        // May panic, that's the fail case for this test.
        let feed = get_mock_rss_feed().await;
        let item_one = feed
            .items
            .into_iter()
            .filter_map(DatedNewsItem::from_source)
            .next()
            .unwrap();
        let expected = DatedNewsItem {
            title: None,
            link: None,
            description: None,
            author: None,
            date: chrono::DateTime::UNIX_EPOCH.into(),
        };
        assert_eq!(expected, item_one);
    }
}
