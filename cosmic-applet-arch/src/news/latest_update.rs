use std::{
    io::{Read, Write},
    path::PathBuf,
};

use anyhow::{anyhow, bail, Context};
use chrono::{DateTime, Local};
use directories::ProjectDirs;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const PACMAN_LOG_PATH: &str = "/var/log/pacman.log";
const LOCAL_LAST_READ_PATH: &str = "last_read";

#[cfg_attr(test, mockall::automock)]
trait ArchInstallation {
    async fn get_pacman_log(&self) -> std::io::Result<String>;
    /// Note - should be provided in RW mode.
    async fn get_local_storage_reader(&self) -> std::io::Result<Box<dyn AsyncRead + Send + Unpin>>;
    async fn get_local_storage_writer(&self)
        -> std::io::Result<Box<dyn AsyncWrite + Send + Unpin>>;
}

pub struct Arch;
impl ArchInstallation for Arch {
    async fn get_pacman_log(&self) -> std::io::Result<String> {
        tokio::fs::read_to_string(PACMAN_LOG_PATH).await
    }
    async fn get_local_storage_reader(&self) -> std::io::Result<Box<dyn AsyncRead + Unpin + Send>> {
        tokio::fs::File::open(platform_local_last_read_path())
            .await
            .map(to_box_reader)
    }
    async fn get_local_storage_writer(
        &self,
    ) -> std::io::Result<Box<dyn AsyncWrite + Unpin + Send>> {
        tokio::fs::File::options()
            .create(true)
            .truncate(true)
            .write(true)
            .open(platform_local_last_read_path())
            .await
            .map(to_box_writer)
    }
}
fn to_box_reader<T: AsyncRead + Unpin + Send + 'static>(t: T) -> Box<dyn Send + AsyncRead + Unpin> {
    Box::new(t) as Box<dyn AsyncRead + Send + Unpin>
}
fn to_box_writer<T: AsyncWrite + Unpin + Send + 'static>(
    t: T,
) -> Box<dyn AsyncWrite + Unpin + Send> {
    Box::new(t) as Box<dyn AsyncWrite + Unpin + Send>
}

fn platform_local_last_read_path() -> PathBuf {
    let proj_dirs = ProjectDirs::from("com", "nick42d", "cosmic-applet-arch").unwrap();
    proj_dirs
        .data_local_dir()
        .to_path_buf()
        .join(LOCAL_LAST_READ_PATH)
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
        .get_local_storage_writer()
        .await
        .context("Error opening last read file")?;
    handle
        .write_all(datetime.to_rfc3339().as_bytes())
        .await
        .context("Error writing last read to disk")
}
async fn get_local_last_read(platform: &impl ArchInstallation) -> anyhow::Result<DateTime<Local>> {
    let mut last_read_string = String::new();
    platform
        .get_local_storage_reader()
        .await
        .context("Error opening last read file")?
        .read_to_string(&mut last_read_string)
        .await
        .context("Error converting last read file to string")?;
    let date_time =
        DateTime::parse_from_rfc3339(&last_read_string).context("Error parsing last read file")?;
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
    let naive_datetime = DateTime::parse_from_str(last_update_str, "%Y-%m-%dT%H:%M:%S%z").context(
        format!("Error parsing pacman log timestamp '{}'", last_update_str),
    )?;
    Ok(DateTime::<Local>::from(naive_datetime))
}
#[cfg(test)]
mod tests {
    use super::{set_local_last_read, to_box_writer};
    use crate::news::latest_update::{
        get_latest_pacman_update, get_local_last_read, to_box_reader, MockArchInstallation,
    };
    use chrono::TimeZone;

    #[tokio::test]
    async fn test_get_latest_pacman_update_mock() {
        let mut mock = MockArchInstallation::new();
        mock.expect_get_pacman_log()
            .returning(|| Ok(include_str!("../../test/pacman.log").to_string()));
        assert_eq!(
            get_latest_pacman_update(&mock).await.unwrap(),
            chrono::FixedOffset::east_opt(8 * 60 * 60)
                .unwrap()
                .with_ymd_and_hms(2024, 2, 5, 22, 2, 13)
                .unwrap()
        );
    }
    #[tokio::test]
    async fn test_get_latest_local_update_mock() {
        let mut mock = MockArchInstallation::new();
        let expected = to_box_reader("2025-02-03T11:24:25+08:00".as_bytes());
        mock.expect_get_local_storage_reader()
            .return_once(|| Ok(expected));
        assert_eq!(
            get_local_last_read(&mock).await.unwrap(),
            chrono::FixedOffset::east_opt(8 * 60 * 60)
                .unwrap()
                .with_ymd_and_hms(2025, 2, 3, 11, 24, 25)
                .unwrap()
        );
    }
    #[tokio::test]
    async fn test_set_latest_local_update_mock() {
        let storage = tempfile::tempdir().unwrap();
        let path = storage.path().join("cosmic-applet-arch-test.txt");
        let writer = to_box_writer(
            tokio::fs::File::options()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&path)
                .await
                .unwrap(),
        );
        let reader = to_box_reader(tokio::fs::File::open(&path).await.unwrap());
        let mut mock = MockArchInstallation::new();
        mock.expect_get_local_storage_writer()
            .return_once(|| Ok(writer));
        mock.expect_get_local_storage_reader()
            .return_once(|| Ok(reader));
        set_local_last_read(
            &mock,
            chrono::FixedOffset::east_opt(8 * 60 * 60)
                .unwrap()
                .with_ymd_and_hms(2025, 2, 3, 11, 24, 25)
                .unwrap()
                .into(),
        )
        .await
        .unwrap();
        assert_eq!(
            get_local_last_read(&mock).await.unwrap(),
            chrono::FixedOffset::east_opt(8 * 60 * 60)
                .unwrap()
                .with_ymd_and_hms(2025, 2, 3, 11, 24, 25)
                .unwrap()
        );
    }
}
