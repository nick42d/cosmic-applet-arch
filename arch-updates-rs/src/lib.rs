use core::str;
use futures::{stream::FuturesUnordered, StreamExt, TryStreamExt};
use raur::Raur;
use srcinfo::Srcinfo;
use std::str::FromStr;
use thiserror::Error;
use tokio::process::Command;
use version_compare::Version;

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

pub enum CheckType<Cache> {
    Online,
    Offline(Cache),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Update {
    pub pkgname: String,
    pub pkgver_cur: String,
    pub pkgrel_cur: String,
    pub pkgver_new: String,
    pub pkgrel_new: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevelUpdate {
    pub pkgname: String,
    pub pkgver_cur: String,
    pub ref_id_new: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Package {
    pub pkgname: String,
    pub pkgver: String,
    pub pkgrel: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageUrl<'a> {
    remote: String,
    protocol: &'a str,
    branch: Option<&'a str>,
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

pub async fn check_updates(check_type: CheckType<()>) -> Result<Vec<Update>> {
    let args = match check_type {
        CheckType::Online => ["--nocolor"].as_slice(),
        CheckType::Offline(()) => ["--nosync", "--nocolor"].as_slice(),
    };
    let output = Command::new("checkupdates").args(args).output().await?;
    str::from_utf8(output.stdout.as_slice())
        .map_err(|_| Error::CheckUpdatesParseFailed)?
        .lines()
        .map(TryInto::try_into)
        .collect()
}

// TODO: Consider case where pkgrel has been bumped.
/// (packages that have an update, cache of packages)
pub async fn check_devel_updates(
    check_type: CheckType<Vec<DevelUpdate>>,
) -> Result<(Vec<DevelUpdate>, Vec<DevelUpdate>)> {
    let devel_packages = get_devel_packages().await?;
    let devel_package_srcinfos = devel_packages
        .into_iter()
        .map(get_aur_srcinfo)
        .collect::<FuturesUnordered<_>>()
        .try_collect::<Vec<_>>()
        .await
        .unwrap();
    let devel_updates = match check_type {
        CheckType::Online => {
            devel_package_srcinfos
                .iter()
                // May be able to avoid the earlier collection.
                .map(|srcinfo| async move {
                    let url = parse_url(srcinfo.base.source.first().unwrap().vec.first().unwrap())
                        .unwrap();
                    let pkgver_cur = srcinfo.base.pkgver.to_owned();
                    let pkgname = srcinfo.pkg.pkgname.to_owned();
                    let ref_id_new = get_head_identifier(url.remote, url.branch).await;
                    DevelUpdate {
                        pkgname,
                        pkgver_cur,
                        ref_id_new,
                    }
                })
                .collect::<FuturesUnordered<_>>()
                .collect::<Vec<_>>()
                .await
        }
        CheckType::Offline(cache) => devel_package_srcinfos
            .iter()
            .map(move |srcinfo| {
                let matching_cached = cache
                    .iter()
                    .find(|cache_package| cache_package.pkgname == srcinfo.pkg.pkgname);
                let ref_id_new = match matching_cached {
                    Some(cache_package) => cache_package.ref_id_new.to_owned(),
                    None => srcinfo.base.pkgver.to_owned(),
                };
                DevelUpdate {
                    pkgname: srcinfo.pkg.pkgname.to_owned(),
                    pkgver_cur: srcinfo.base.pkgver.to_owned(),
                    ref_id_new,
                }
            })
            .collect(),
    };
    Ok((
        devel_updates
            .iter()
            .filter(|update| update.pkgver_cur.contains(&update.ref_id_new))
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>(),
        devel_updates,
    ))
}

// TODO: Consider if devel packages should be filtered entirely.
/// (packages that have an update, cache of packages)
pub async fn check_aur_updates(
    check_type: CheckType<Vec<Update>>,
) -> Result<(Vec<Update>, Vec<Update>)> {
    let old = get_old_aur_packages().await?;
    let updated_cache: Vec<Update> = match check_type {
        CheckType::Online => {
            let aur = raur::Handle::new();
            aur.info(
                old.iter()
                    .map(|pkg| pkg.pkgname.to_owned())
                    .collect::<Vec<_>>()
                    .as_slice(),
            )
            .await
            .map_err(|_| Error::GetNewAurPackagesFailed)?
            .into_iter()
            .filter_map(|new| {
                let matching_old = &old.iter().find(|old| old.pkgname == new.name)?.clone();
                let (pkgver_new, pkgrel_new) = parse_version(new.version).unwrap();
                Some(Update {
                    pkgname: matching_old.pkgname.to_owned(),
                    pkgver_cur: matching_old.pkgver.to_owned(),
                    pkgrel_cur: matching_old.pkgrel.to_owned(),
                    pkgver_new,
                    pkgrel_new,
                })
            })
            .collect()
        }
        CheckType::Offline(cache) => old
            .iter()
            .map(|old_package| {
                let matching_cached = cache
                    .iter()
                    .find(|cache_package| cache_package.pkgname == old_package.pkgname);
                let (pkgver_new, pkgrel_new) = match matching_cached {
                    Some(cache_package) => (
                        cache_package.pkgver_new.to_owned(),
                        cache_package.pkgrel_new.to_owned(),
                    ),
                    None => (old_package.pkgver.to_owned(), old_package.pkgrel.to_owned()),
                };
                Update {
                    pkgname: old_package.pkgname.to_owned(),
                    pkgver_cur: old_package.pkgver.to_owned(),
                    pkgrel_cur: old_package.pkgrel.to_owned(),
                    pkgver_new,
                    pkgrel_new,
                }
            })
            .collect(),
    };
    Ok((
        updated_cache
            .iter()
            .filter(|package| {
                // VCS packages will fail here! But this is likely desired behaviour.
                let Some(pkgver_new) = Version::from(&package.pkgver_new) else {
                    return false;
                };
                let Some(pkgver_old) = Version::from(&package.pkgver_cur) else {
                    return false;
                };
                pkgver_new > pkgver_old
                    || (pkgver_new == pkgver_old && package.pkgrel_new > package.pkgrel_cur)
            })
            .cloned()
            .collect(),
        updated_cache,
    ))
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

/// Parse output of pacman -Qm into a package.
fn parse_pacman_qm(line: String) -> Result<Package> {
    let (pkgname, rest) = line.split_once(' ').unwrap();
    let (pkgver, pkgrel) = parse_version(rest)?;
    Ok(Package {
        pkgname: pkgname.to_owned(),
        pkgver,
        pkgrel,
    })
}

/// Parse output of a combined pkgrel-pkgver.
fn parse_version(version: impl AsRef<str>) -> Result<(String, String)> {
    let (pkgver, pkgrel) = version.as_ref().rsplit_once('-').unwrap();
    Ok((pkgver.into(), pkgrel.into()))
}

async fn get_old_aur_packages() -> Result<Vec<Package>> {
    let (ignored_packages, output) = tokio::join!(
        get_ignored_packages(),
        Command::new("pacman").arg("-Qm").output()
    );
    let ignored_packages = ignored_packages?;
    str::from_utf8(output?.stdout.as_slice())
        .map_err(|_| Error::GetIgnoredPackagesFailed)?
        .lines()
        // Filter out any ignored packages
        .filter(|line| {
            !ignored_packages
                .iter()
                .any(|ignored_package| line.contains(ignored_package))
        })
        .map(ToString::to_string)
        .map(parse_pacman_qm)
        .collect()
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

async fn get_aur_srcinfo(pkgname: String) -> Result<Srcinfo> {
    let url = format!("https://aur.archlinux.org/cgit/aur.git/plain/.SRCINFO?h={pkgname}");
    let raw = reqwest::get(url).await.unwrap().text().await.unwrap();
    Ok(Srcinfo::from_str(&raw).unwrap())
}

async fn get_head_identifier(url: String, branch: Option<&str>) -> String {
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
    .get(0..7)
    .unwrap()
    .to_string()
}

// This is from paru (GPL3)
fn parse_url(source: &str) -> Option<PackageUrl> {
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

    Some(PackageUrl {
        remote,
        protocol,
        branch,
    })
}

#[cfg(test)]
mod tests {
    use crate::{
        check_aur_updates, check_devel_updates, check_updates, get_aur_srcinfo, get_devel_packages,
        get_head_identifier, get_old_aur_packages, parse_pacman_qm, parse_url, parse_version,
        CheckType, DevelUpdate, Package, PackageUrl, Update,
    };

    #[tokio::test]
    async fn test_check_updates() {
        let online = check_updates(CheckType::Online).await.unwrap();
        let offline = check_updates(CheckType::Offline(())).await.unwrap();
        assert_eq!(online, offline);
    }
    #[tokio::test]
    async fn test_check_aur_updates() {
        let (online, cache) = check_aur_updates(CheckType::Online).await.unwrap();
        let (offline, _) = check_aur_updates(CheckType::Offline(cache)).await.unwrap();
        assert_eq!(online, offline);
        eprintln!("aur {:#?}", online);
    }
    #[tokio::test]
    async fn test_check_devel_updates() {
        let (online, cache) = check_devel_updates(CheckType::Online).await.unwrap();
        let (offline, _) = check_devel_updates(CheckType::Offline(cache))
            .await
            .unwrap();
        assert_eq!(online, offline);
        eprintln!("devel {:#?}", online);
    }

    #[tokio::test]
    async fn test_get_srcinfo() {
        get_aur_srcinfo("hyprlang-git".to_string()).await.unwrap();
    }
    #[tokio::test]
    async fn test_get_url() {
        let srcinfo = get_aur_srcinfo("hyprlang-git".to_string()).await.unwrap();
        let url = srcinfo.base.source.first().unwrap().vec.first().unwrap();
        parse_url(url).unwrap();
    }
    #[tokio::test]
    async fn test_get_head() {
        let srcinfo = get_aur_srcinfo("hyprutils-git".to_string()).await.unwrap();
        let url = srcinfo.base.source.first().unwrap().vec.first().unwrap();
        let url_parsed = parse_url(url).unwrap();
        let x = get_head_identifier(url_parsed.remote, url_parsed.branch).await;
        eprintln!("{}", x)
    }

    #[test]
    fn test_parse_url() {
        let url = parse_url(
            "paper-icon-theme::git+https://github.com/snwh/paper-icon-theme.git#branch=main",
        )
        .unwrap();
        let expected = PackageUrl {
            remote: "https://github.com/snwh/paper-icon-theme.git".to_string(),
            protocol: "https",
            branch: Some("main"),
        };
        assert_eq!(url, expected);
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
    #[test]
    fn test_parse_pacman_qm() {
        let update =
            parse_pacman_qm("winetricks-git 20240105.r47.g72b934e1-2".to_string()).unwrap();
        let expected = Package {
            pkgname: "winetricks-git".to_string(),
            pkgver: "20240105.r47.g72b934e1".to_string(),
            pkgrel: "2".to_string(),
        };
        assert_eq!(update, expected);
    }
    #[test]
    fn test_parse_version() {
        let actual = parse_version("20-240105.r47.g72b934e1-2").unwrap();
        let expected = ("20-240105.r47.g72b934e1".to_string(), "2".to_string());
        assert_eq!(actual, expected);
    }
}
