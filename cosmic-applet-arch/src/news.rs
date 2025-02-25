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
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    const PACMAN_LOG_PATH: &str = "/var/log/pacman.log";
    const LOCAL_LAST_READ_PATH: &str = "last_read";

    #[cfg_attr(test, mockall::automock)]
    trait ArchInstallation {
        async fn get_pacman_log(&self) -> std::io::Result<String>;
        /// Note - should be provided in RW mode.
        async fn get_local_update_timestamp_file(&self) -> std::io::Result<tokio::fs::File>;
    }

    struct Arch;
    impl ArchInstallation for Arch {
        async fn get_pacman_log(&self) -> std::io::Result<String> {
            tokio::fs::read_to_string(PACMAN_LOG_PATH).await
        }
        async fn get_local_update_timestamp_file(&self) -> std::io::Result<tokio::fs::File> {
            let proj_dirs = ProjectDirs::from("com", "nick42d", "cosmic-applet-arch").unwrap();
            tokio::fs::File::options()
                .write(true)
                .open(
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
    pub async fn set_local_last_read(
        platform: &impl ArchInstallation,
        datetime: DateTime<Local>,
    ) -> anyhow::Result<()> {
        let mut handle = platform
            .get_local_update_timestamp_file()
            .await
            .context("Error opening last read file")?;
        handle
            .write_all(datetime.to_rfc3339().as_bytes())
            .await
            .context("Error writing last read to disk")
    }
    async fn get_local_last_read(
        platform: &impl ArchInstallation,
    ) -> anyhow::Result<DateTime<Local>> {
        let mut last_read_string = String::new();
        platform
            .get_local_update_timestamp_file()
            .await
            .context("Error opening last read file")?
            .read_to_string(&mut last_read_string)
            .await
            .context("Error converting last read file to string")?;
        let date_time = DateTime::parse_from_rfc3339(&last_read_string)
            .context("Error parsing last read file")?;
        Ok(DateTime::<Local>::from(date_time))
    }
    async fn get_latest_pacman_update(
        platform: &impl ArchInstallation,
    ) -> anyhow::Result<DateTime<Local>> {
        let log = &platform
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
        let naive_datetime = DateTime::parse_from_str(last_update_str, "%Y-%m-%dT%H:%M:%S%z")
            .context(format!(
                "Error parsing pacman log timestamp '{}'",
                last_update_str
            ))?;
        Ok(DateTime::<Local>::from(naive_datetime))
    }
    #[cfg(test)]
    mod tests {
        use chrono::TimeZone;

        use crate::news::latest_update::{
            get_latest_pacman_update, get_local_last_read, MockArchInstallation,
        };

        #[tokio::test]
        async fn test_get_latest_pacman_update_mock() {
            let mut mock = MockArchInstallation::new();
            mock.expect_get_pacman_log()
                .returning(|| Ok(include_str!("../test/pacman.log").to_string()));
            assert_eq!(
                get_latest_pacman_update(&mock).await.unwrap(),
                chrono::FixedOffset::east_opt(8 * 60 * 60)
                    .unwrap()
                    .with_ymd_and_hms(2025, 2, 5, 22, 2, 13)
                    .unwrap()
            );
        }
        #[tokio::test]
        async fn test_get_latest_local_update_mock() {
            let mut mock = MockArchInstallation::new();
            mock.expect_get_local_update_timestamp_file()
                .returning(|| Ok("2025-02-03T11:24:25+08:00".to_string()));
            assert_eq!(
                get_local_last_read(&mock).await.unwrap(),
                chrono::FixedOffset::east_opt(8 * 60 * 60)
                    .unwrap()
                    .with_ymd_and_hms(2025, 2, 3, 11, 24, 25)
                    .unwrap()
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
    async fn get_arch_news_feed(&self) -> Result<Channel, NewsError>;
}

struct Network;

const ARCH_NEWS_FEED_URL: &str = "https://archlinux.org/feeds/news/";

impl ArchNewsFeed for Network {
    async fn get_arch_news_feed(&self) -> Result<Channel, NewsError> {
        let content = reqwest::get(ARCH_NEWS_FEED_URL).await?.bytes().await?;
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
            pub_date,
            dublin_core_ext,
            ..
        } = item;
        // Should not having a date be an error?
        let date = pub_date?;
        let date = chrono::DateTime::parse_from_rfc2822(&date).ok()?;
        let author = dublin_core_ext.map(|dc| dc.creators.into_iter().next().unwrap());
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
    feed: &impl ArchNewsFeed,
    last_updated_dt: chrono::DateTime<chrono::FixedOffset>,
) -> Result<Vec<DatedNewsItem>, NewsError> {
    let feed = feed.get_arch_news_feed().await?;
    Ok(feed
        .items
        .into_iter()
        .filter_map(DatedNewsItem::from_source)
        .filter(|item| item.date < last_updated_dt)
        .collect())
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, TimeZone};

    use super::*;
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
    async fn test_get_latest_news_multiple() {
        let mock = get_mock().await;
        let latest_news = get_latest_arch_news(&mock, chrono::Local::now().into())
            .await
            .unwrap();
        assert_eq!(latest_news.len(), 3);
    }
    #[tokio::test]
    async fn test_get_latest_news_one() {
        let mock = get_mock().await;
        let latest_news = get_latest_arch_news(&mock, chrono::Local::now().into())
            .await
            .unwrap();
        let expected = DatedNewsItem {
            title: todo!(),
            link: todo!(),
            description: todo!(),
            author: todo!(),
            date: todo!(),
        };
        assert_eq!(latest_news[0], expected);
    }
    #[tokio::test]
    async fn test_feed_has_all_items() {
        // May panic, that's the fail case for this test.
        let feed = get_mock().await.get_arch_news_feed().await.unwrap();
        let items: Vec<_> = feed
            .items
            .into_iter()
            .filter_map(DatedNewsItem::from_source)
            .collect();
        assert!(items.len() == 10);
    }

    #[tokio::test]
    async fn test_feed_has_specific_item() {
        // May panic, that's the fail case for this test.
        let feed = get_mock().await.get_arch_news_feed().await.unwrap();
        let item_one = feed
            .items
            .into_iter()
            .filter_map(DatedNewsItem::from_source)
            .next()
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
