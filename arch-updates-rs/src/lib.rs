use core::str;
use thiserror::Error;
use tokio::process::Command;

#[derive(Error, Debug)]
pub enum Error {
    #[error("checkupdates failed")]
    Io(#[from] std::io::Error),
    // #[error("the data for key `{0}` is not available")]
    // Redaction(String),
    // #[error("invalid header (expected {expected:?}, found {found:?})")]
    // InvalidHeader {
    //     expected: String,
    //     found: String,
    // },
    // #[error("unknown data store error")]
    // Unknown,
    #[error("Failed to parse update from checkupdates")]
    CheckUpdatesParseFailed,
}
pub type Result<T> = std::result::Result<T, Error>;

pub enum CheckType {
    Online,
    Offline,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Update {
    pub pkgname: String,
    pub pkgver_cur: String,
    pub pkgrel_cur: String,
    pub pkgver_new: String,
    pub pkgrel_new: String,
}

impl TryFrom<&str> for Update {
    type Error = Error;
    /// Example input: libadwaita 1:1.6.0-1 -> 1:1.6.1-1
    fn try_from(value: &str) -> Result<Self> {
        /// (pkgver, pkgrel)
        fn parse_pkgvers(val: &str) -> Result<(String, String)> {
            if let Some((ver, rel)) = val.rsplit_once('-') {
                return Ok((ver.to_string(), rel.to_string()));
            }
            Err(Error::CheckUpdatesParseFailed)
        }
        let mut iter = value.split(' ');
        let pkgname = iter
            .next()
            .ok_or(Error::CheckUpdatesParseFailed)?
            .to_string();
        let (pkgver_cur, pkgrel_cur) =
            parse_pkgvers(iter.next().ok_or(Error::CheckUpdatesParseFailed)?)?;
        let (pkgver_new, pkgrel_new) =
            parse_pkgvers(iter.nth(1).ok_or(Error::CheckUpdatesParseFailed)?)?;
        Ok(Self {
            pkgname,
            pkgver_cur,
            pkgrel_cur,
            pkgver_new,
            pkgrel_new,
        })
    }
}

pub async fn check_updates(check_type: CheckType) -> Result<Vec<Update>> {
    let args = match check_type {
        CheckType::Online => ["--nocolor"].as_slice(),
        CheckType::Offline => ["--nosync", "--nocolor"].as_slice(),
    };
    let output = Command::new("checkupdates").args(args).output().await?;
    str::from_utf8(output.stdout.as_slice())
        .map_err(|_| Error::CheckUpdatesParseFailed)?
        .lines()
        .map(TryInto::try_into)
        .collect::<Result<Vec<_>>>()
}

#[cfg(test)]
mod tests {
    use crate::{check_updates, CheckType, Update};

    #[tokio::test]
    async fn test_check_updates() {
        let online = check_updates(CheckType::Online).await.unwrap();
        let offline = check_updates(CheckType::Offline).await.unwrap();
        assert_eq!(online, offline);
    }

    #[test]
    fn test_parse_update() {
        let update = Update::try_from("libadwaita 1:1.6.0-1 -> 1:1.6.1-2").unwrap();
        let expected = Update {
            pkgname: "libadwaita".to_string(),
            pkgver_cur: "1:1.6.0".to_string(),
            pkgrel_cur: "1".to_string(),
            pkgver_new: "1:1.6.1".to_string(),
            pkgrel_new: "2".to_string(),
        };
        assert_eq!(update, expected);
    }
}
