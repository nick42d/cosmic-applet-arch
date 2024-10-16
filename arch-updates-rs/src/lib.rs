use core::str;
use raur::Raur;
use std::{io::Stdout, process::Stdio};
use thiserror::Error;
use tokio::process::Command;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error running command")]
    Io(#[from] std::io::Error),
    #[error("Failed to parse update from checkupdates")]
    CheckUpdatesParseFailed,
    #[error("Failed to get ignored packages")]
    GetIgnoredPackagesFailed,
    #[error("Failed to get new aur packages")]
    GetNewAurPackagesFailed,
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

pub struct Package {
    pub pkgname: String,
    pub pkgver: String,
    pub pkgrel: String,
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
        .collect()
}

async fn get_ignored_packages() -> Result<Vec<String>> {
    // Considered pacmanconf crate here, but it's sync, and does the same thing
    // under the hood (runs pacman-conf) as a Command.
    let output = Command::new("pacman-conf")
        .arg("IgnorePkg")
        .output()
        .await?;
    Ok(str::from_utf8(output.stdout.as_slice())
        .map_err(|_| Error::GetIgnoredPackagesFailed)?
        .lines()
        .map(ToString::to_string)
        .collect())
}

async fn get_old_aur_packages() -> Result<Vec<String>> {
    let (ignored_packages, output) = tokio::join!(
        get_ignored_packages(),
        Command::new("pacman").arg("-Qm").output()
    );
    let ignored_packages = ignored_packages?;
    Ok(str::from_utf8(output?.stdout.as_slice())
        .map_err(|_| Error::GetIgnoredPackagesFailed)?
        .lines()
        // Filter out any ignored packages
        .filter(|line| {
            ignored_packages
                .iter()
                .any(|ignored_package| line.contains(ignored_package))
        })
        .map(ToString::to_string)
        .collect())
}

async fn get_new_aur_packages(old_packages: Vec<String>) -> Result<Vec<raur::Package>> {
    let aur = raur::Handle::new();
    aur.info(old_packages.as_slice())
        .await
        .map_err(|_| Error::GetNewAurPackagesFailed)
}

async fn get_devel_packages() -> Result<Vec<String>> {
    const DEVEL_SUFFIXES: [&str; 1] = ["-git"];
    let (ignored_packages, output_unfiltered) = tokio::join!(
        get_ignored_packages(),
        Command::new("pacman").arg("-Qm").output()
    );
    let ignored_packages = ignored_packages?;
    Ok(str::from_utf8(output_unfiltered?.stdout.as_slice())
        .map_err(|_| Error::GetIgnoredPackagesFailed)?
        .lines()
        // Only include packages with DEVEL_SUFFIXES.
        .filter(|line| {
            DEVEL_SUFFIXES
                .iter()
                .any(|suffix| line.to_lowercase().contains(suffix))
        })
        // Filter out any ignored packages
        .filter(|line| {
            ignored_packages
                .iter()
                .any(|ignored_package| line.contains(ignored_package))
        })
        .map(ToString::to_string)
        .collect())
}

async fn get_pkgbuild(pkgname: String) -> Result<Vec<String>> {
    let custom_pkgbuild_vars = [
        "_gitname=",
        "_githubuser=",
        "_githubrepo=",
        "_gitcommit=",
        "url=",
        "_pkgname=",
        "_gitdir=",
        "_repo_name=",
        "_gitpkgname=",
        "source_dir=",
        "_name=",
    ];
    let url = format!("https://aur.archlinux.org/cgit/aur.git/plain/PKGBUILD?h={pkgname}");
    Ok(reqwest::get(url)
        .await
        .unwrap()
        .text()
        .await
        .unwrap()
        .lines()
        .filter(|line| {
            custom_pkgbuild_vars
                .iter()
                .any(|var| line.to_lowercase().contains(var))
        })
        .map(ToString::to_string)
        .collect())
}

#[cfg(test)]
mod tests {
    use crate::{check_updates, get_pkgbuild, CheckType, Update};
    use raur::Raur;

    #[tokio::test]
    async fn test_check_updates() {
        let online = check_updates(CheckType::Online).await.unwrap();
        let offline = check_updates(CheckType::Offline).await.unwrap();
        assert_eq!(online, offline);
    }
    #[tokio::test]
    async fn test_download() {
        // let out = raur::Handle::new()
        //     .raw_info(&["hyprlang-git"])
        //     .await
        //     .unwrap();
        let out = get_pkgbuild("hyprlang-git".to_string()).await.unwrap();
        eprintln!("{:?}", out)
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
