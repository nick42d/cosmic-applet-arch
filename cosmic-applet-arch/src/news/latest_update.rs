use super::WarnedResult;
use crate::core::proj_dirs;
use anyhow::{anyhow, Context};
use chrono::{DateTime, FixedOffset};
use std::path::PathBuf;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const PACMAN_LOG_PATH: &str = "/var/log/pacman.log";
const LOCAL_LAST_READ_PATH: &str = "last_read";

#[cfg_attr(test, mockall::automock)]
pub trait ArchInstallation {
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
        tokio::fs::File::open(platform_local_last_read_path()?)
            .await
            .map(to_box_reader)
    }
    async fn get_local_storage_writer(
        &self,
    ) -> std::io::Result<Box<dyn AsyncWrite + Unpin + Send>> {
        let path = platform_local_last_read_path()?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::File::options()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)
            .await
            .map(to_box_writer)
    }
}

/// Helper function
fn to_box_reader<T: AsyncRead + Unpin + Send + 'static>(t: T) -> Box<dyn Send + AsyncRead + Unpin> {
    Box::new(t) as Box<dyn AsyncRead + Send + Unpin>
}
/// Helper function
fn to_box_writer<T: AsyncWrite + Unpin + Send + 'static>(
    t: T,
) -> Box<dyn AsyncWrite + Unpin + Send> {
    Box::new(t) as Box<dyn AsyncWrite + Unpin + Send>
}

fn platform_local_last_read_path() -> std::io::Result<PathBuf> {
    Ok(proj_dirs()
        .ok_or(std::io::Error::other(
            "Unable to obtain a local data storage directory",
        ))?
        .data_local_dir()
        .to_path_buf()
        .join(LOCAL_LAST_READ_PATH))
}

pub async fn get_latest_update(
    platform: &impl ArchInstallation,
) -> WarnedResult<Option<DateTime<FixedOffset>>, String, anyhow::Error> {
    let (local_last_read, latest_pacman_update) = futures::future::join(
        get_local_last_read(platform),
        get_latest_pacman_update(platform),
    )
    .await;
    match (local_last_read, latest_pacman_update) {
        (local_dt, Ok(None)) => {
            WarnedResult::from_result(local_dt.context("Error determining last update")).map(Some)
        },
        (Ok(local_dt), Ok(Some(pacman_dt))) => {
            WarnedResult::Ok(Some(local_dt.max(pacman_dt)))
        },
        (Ok(local_dt), Err(e)) => {
            WarnedResult::Warning(Some(local_dt), format!("Recieved an error determining last update, but there was a fallback method that could be used {e}"))
        },
        // In this case, we've got a pacman dt but no local dt. This is normal enough not to need to warn.
        (Err(_), Ok(pacman_dt)) => WarnedResult::Ok(pacman_dt),
        (Err(e1), Err(e2)) => WarnedResult::Err(anyhow!("Errors determining last update, {e1}, {e2}"))
    }
}
pub async fn set_local_last_read(
    platform: &impl ArchInstallation,
    datetime: DateTime<FixedOffset>,
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
async fn get_local_last_read(
    platform: &impl ArchInstallation,
) -> anyhow::Result<DateTime<FixedOffset>> {
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
    Ok(date_time)
}
async fn get_latest_pacman_update(
    platform: &impl ArchInstallation,
) -> anyhow::Result<Option<DateTime<FixedOffset>>> {
    let log = &platform
        .get_pacman_log()
        .await
        .context("Error reading pacman log")?;
    let Some(last_update_line) = log
        .lines()
        .filter(|line| line.contains("starting full system upgrade"))
        .next_back()
    else {
        return Ok(None);
    };
    let last_update_str = last_update_line
        .trim_start_matches('[')
        .split(']')
        .next()
        .context("Error parsing pacman log")?;
    let naive_datetime = DateTime::parse_from_str(last_update_str, "%Y-%m-%dT%H:%M:%S%z").context(
        format!("Error parsing pacman log timestamp '{}'", last_update_str),
    )?;
    Ok(Some(naive_datetime))
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
            get_latest_pacman_update(&mock).await.unwrap().unwrap(),
            chrono::FixedOffset::east_opt(8 * 60 * 60)
                .unwrap()
                .with_ymd_and_hms(2024, 2, 5, 22, 2, 13)
                .unwrap()
        );
    }
    #[tokio::test]
    async fn test_pacman_log_with_no_update_shouldnt_error() {
        let mut mock = MockArchInstallation::new();
        mock.expect_get_pacman_log()
            .returning(|| Ok(include_str!("../../test/pacman-no-update.log").to_string()));
        assert!(get_latest_pacman_update(&mock).await.unwrap().is_none());
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
                .unwrap(),
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
