use core::str;
use raur::Raur;
use srcinfo::Srcinfo;
use std::{collections::HashMap, io::Stdout, process::Stdio, str::FromStr};
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

async fn get_aur_pkgbuild(pkgname: String) -> Result<String> {
    let url = format!("https://aur.archlinux.org/cgit/aur.git/plain/PKGBUILD?h={pkgname}");
    Ok(reqwest::get(url).await.unwrap().text().await.unwrap())
}

async fn get_aur_srcinfo(pkgname: String) -> Result<Srcinfo> {
    let url = format!("https://aur.archlinux.org/cgit/aur.git/plain/.SRCINFO?h={pkgname}");
    let raw = reqwest::get(url).await.unwrap().text().await.unwrap();
    Ok(Srcinfo::from_str(&raw).unwrap())
}

async fn get_head(url: String, branch: Option<&str>) -> String {
    str::from_utf8(
        Command::new("git")
            .args(["ls-remote", &url, branch.unwrap_or("HEAD")])
            .output()
            .await
            .unwrap()
            .stdout
            .as_ref(),
    )
    .unwrap()
    .to_string()
}

// This is from paru (GPL3)
fn parse_url(source: &str) -> Option<(String, &str, Option<&str>)> {
    let url = source.splitn(2, "::").last().unwrap();

    if !url.starts_with("git") || !url.contains("://") {
        return None;
    }

    let mut split = url.splitn(2, "://");
    let protocol = split.next().unwrap();
    let protocol = protocol.rsplit('+').next().unwrap();
    let rest = split.next().unwrap();

    let mut split = rest.splitn(2, '#');
    let remote = split.next().unwrap();
    let remote = remote.split_once('?').map_or(remote, |x| x.0);
    let remote = format!("{}://{}", protocol, remote);

    let branch = if let Some(fragment) = split.next() {
        let fragment = fragment.split_once('?').map_or(fragment, |x| x.0);
        let mut split = fragment.splitn(2, '=');
        let frag_type = split.next().unwrap();

        match frag_type {
            "commit" | "tag" => return None,
            "branch" => split.next(),
            _ => None,
        }
    } else {
        None
    };

    Some((remote, protocol, branch))
}

#[cfg(test)]
mod tests {
    use crate::{check_updates, get_aur_srcinfo, get_head, parse_url, CheckType, Update};

    #[tokio::test]
    async fn test_check_updates() {
        let online = check_updates(CheckType::Online).await.unwrap();
        let offline = check_updates(CheckType::Offline).await.unwrap();
        assert_eq!(online, offline);
    }
    #[tokio::test]
    async fn test_get_srcinfo() {
        get_aur_srcinfo("hyprlang-git".to_string()).await.unwrap();
    }
    #[tokio::test]
    async fn test_get_url() {
        let srcinfo = get_aur_srcinfo("hyprlang-git".to_string()).await.unwrap();
        let url = srcinfo.base.source.first().unwrap().vec.first().unwrap();
        let x = parse_url(url).unwrap();
        eprintln!("{:?}", x)
    }
    #[tokio::test]
    async fn test_get_head() {
        let srcinfo = get_aur_srcinfo("hyprlang-git".to_string()).await.unwrap();
        let url = srcinfo.base.source.first().unwrap().vec.first().unwrap();
        let url_parsed = parse_url(url).unwrap();
        let x = get_head(url_parsed.0, url_parsed.2).await;
        eprintln!("{}", x)
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
