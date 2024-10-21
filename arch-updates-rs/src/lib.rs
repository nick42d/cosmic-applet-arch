//! # arch_updates_rs
//! Library to query arch linux packaging tools to see if updates are available.
//! Designed for cosmic-applet-arch, but could be used in similar apps as well.
//! # Usage
//! ```no_run
//!
//! #[tokio::main]
//! pub async fn main() -> Result<(), arch_updates_ys::Error> {
//!     let (pacman, aur, devel) = tokio::join!(
//!         let pacman = check_updates(CheckType::Online),
//!         let aur = check_aur_updates(CheckType::Online),
//!         let devel = check_devel_updates(CheckType::Online),
//!     ).await;
//!
//!     let cookie_path = std::path::Path::new("./cookie.txt");
//!     let yt = ytmapi_rs::YtMusic::from_cookie_file(cookie_path).await?;
//!     yt.get_search_suggestions("Beatles").await?;
//!     let result = yt.get_search_suggestions("Beatles").await?;
//!     println!("{:?}", result);
//!     Ok(())
//! }
//! ```
use core::str;
use futures::{stream::FuturesOrdered, StreamExt, TryStreamExt};
use raur::Raur;
use srcinfo::Srcinfo;
use std::{
    io,
    str::{FromStr, Utf8Error},
};
use thiserror::Error;
use tokio::process::Command;
use version_compare::Version;

/// Packages ending with one of the devel suffixes will be checked against the
/// repository, as well as just the pkgver and pkgrel.
pub const DEVEL_SUFFIXES: [&str; 1] = ["-git"];

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error running command `{0}`")]
    Io(#[from] io::Error),
    #[error("Web error `{0}`")]
    Web(#[from] reqwest::Error),
    #[error("Error parsing stdout from command")]
    Stdout(#[from] Utf8Error),
    #[error("Failed to get ignored packages")]
    GetIgnoredPackagesFailed,
    #[error("Head identifier too short")]
    HeadIdentifierTooShort,
    #[error("Failed to get package from AUR `{0:?}`")]
    /// # Note
    /// Due to the API design, it's not always possible to know the name of the
    /// aur package we failed to get.
    GetAurPackageFailed(Option<String>),
    #[error("Error parsing .SRCINFO")]
    ParseErrorSrcinfo(#[from] srcinfo::Error),
    #[error("Failed to parse update from checkupdates string: `{0}`")]
    ParseErrorCheckUpdates(String),
    #[error("Failed to parse update from pacman string: `{0}`")]
    ParseErrorPacman(String),
    #[error("Failed to parse pkgver and pkgrel from string `{0}`")]
    ParseErrorPkgverPkgrel(String),
}

pub enum CheckType {
    Online,
    Offline,
}

#[derive(Debug)]
pub struct UpdateCheckOutcome<T> {
    pub updates: Result<Vec<T>>,
    pub cache: Result<Vec<T>>,
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
    pub pkgrel_cur: String,
    /// When checking a devel update, we don't get a pkgver/pkgrel so-to-speak,
    /// we instead get the github ref.
    pub ref_id_new: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Package {
    pub pkgname: String,
    pub pkgver: String,
    pub pkgrel: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PackageUrl<'a> {
    remote: String,
    protocol: &'a str,
    branch: Option<&'a str>,
}

/// Use the `checkupdates` function to check if any pacman-managed packages have
/// updates due.
/// # Usage
/// ```no_run
/// # async {
/// let online = check_updates(CheckType::Online).await.unwrap();
/// let offline = check_updates(CheckType::Offline).await.unwrap();
/// assert_eq!(online, offline);
/// // Run `sudo pacman -Syu` in the terminal
/// let offline = check_updates(CheckType::Offline).await.unwrap();
/// assert!(offline.is_empty());
/// # };
pub async fn check_updates(check_type: CheckType) -> Result<Vec<Update>> {
    let args = match check_type {
        CheckType::Online => ["--nocolor"].as_slice(),
        CheckType::Offline => ["--nosync", "--nocolor"].as_slice(),
    };
    let output = Command::new("checkupdates").args(args).output().await?;
    str::from_utf8(output.stdout.as_slice())?
        .lines()
        .map(parse_update)
        .collect()
}

/// Check if any packages ending in `DEVEL_SUFFIXES` have updates to their
/// source repositories.
///
/// Online version - this function checks the network.
/// Returns a tuple of:
///  - Packages that are not up to date.
///  - Latest version of all devel packages - for offline use.
///
/// # Notes
///  - For this to be accurate, it's reliant on each devel package having only
///    one source URL. If this is not the case, the function will produce a
///    DevelUpdate for each source url, and may assume one or more are out of
///    date.
///  - This is also reliant on VCS packages being good
///    citizens and following the VCS Packaging Guidelines.
///    https://wiki.archlinux.org/title/VCS_package_guidelines
/// # Usage
/// ```no_run
/// # async {
/// let (updates, cache) = check_devel_updates_online().await.unwrap();
/// // Run `paru -Syu` in the terminal
/// let updates = check_devel_updates_online().await.unwrap();
/// assert!(updates.is_empty());
/// # };
pub async fn check_devel_updates_online() -> Result<(Vec<DevelUpdate>, Vec<DevelUpdate>)> {
    let devel_packages = get_devel_packages().await?;
    let devel_updates = futures::stream::iter(devel_packages.into_iter())
        .then(|pkg| async move {
            compile_error!("Remove this unwrap!")
            let srcinfo = get_aur_srcinfo(&pkg.pkgname).await.unwrap();
            let source = srcinfo.base.source;
            source
                .into_iter()
                .flat_map(move |arch| arch.vec.into_iter())
                .filter_map(|url| {
                    let url = parse_url(&url)?;
                    let PackageUrl { remote, branch, .. } = url;
                    // This allocation isn't ideal, but it's here to work around lifetime issues
                    // with nested streams that I've been unable to resolve. Spent a few hours on it
                    // so far!
                    Some((remote, branch.map(ToString::to_string)))
                })
                .map(move |(remote, branch)| {
                    let pkgver_cur = pkg.pkgver.to_owned();
                    let pkgrel_cur = pkg.pkgrel.to_owned();
                    let pkgname = pkg.pkgname.to_owned();
                    async move {
                        let ref_id_new = get_head_identifier(remote, branch.as_deref()).await?;
                        Ok::<_, crate::Error>(DevelUpdate {
                            pkgname,
                            pkgver_cur,
                            ref_id_new,
                            pkgrel_cur,
                        })
                    }
                })
                .collect::<FuturesOrdered<_>>()
        })
        .flatten()
        .try_collect::<Vec<_>>()
        .await?;
    Ok((
        devel_updates
            .iter()
            .filter(|update| devel_update_due(update))
            .cloned()
            .collect::<Vec<_>>(),
        devel_updates,
    ))
}

/// Check if any packages ending in `DEVEL_SUFFIXES` have updates to their
/// source repositories.
///
/// Offline version - this function needs a reference to the latest version of
/// all devel packages (returned from `check_devel_updates_online()`.
/// # Usage
/// ```no_run
/// # async {
/// let (online, cache) = check_devel_updates_online().await.unwrap();
/// let offline = check_devel_updates_online(&cache).await.unwrap();
/// assert_eq!(online, offline);
/// // Run `paru -Syu` in the terminal
/// let offline = check_devel_updates_online(&cache).await.unwrap();
/// assert!(offline.is_empty());
/// # };
pub async fn check_devel_updates_offline(cache: &[DevelUpdate]) -> Result<Vec<DevelUpdate>> {
    let devel_packages = get_devel_packages().await?;
    let devel_updates = devel_packages
        .iter()
        .flat_map(|package| {
            cache
                .iter()
                .filter(|cache_package| cache_package.pkgname == package.pkgname)
                .map(move |cache_package| DevelUpdate {
                    pkgname: package.pkgname.to_owned(),
                    pkgver_cur: package.pkgver.to_owned(),
                    pkgrel_cur: package.pkgrel.to_owned(),
                    ref_id_new: cache_package.ref_id_new.to_owned(),
                })
        })
        .filter(devel_update_due)
        .collect();
    Ok(devel_updates)
}

/// Check if any AUR packages have updates to their pkgver-pkgrel.
///
/// Online version - this function checks the network.
/// Returns a tuple of:
///  - Packages that are not up to date.
///  - Latest version of all aur packages - for offline use.
///
/// # Notes
///  - Locally installed packages that aren't in the AUR are currently not
///    implemented and may return an error.
/// # Usage
/// ```no_run
/// # async {
/// let (updates, cache) = check_aur_updates_online().await.unwrap();
/// // Run `paru -Syu` in the terminal
/// let updates = check_aur_updates_online().await.unwrap();
/// assert!(updates.is_empty());
/// # };
pub async fn check_aur_updates_online() -> Result<(Vec<Update>, Vec<Update>)> {
    let old = get_aur_packages().await?;
    let aur = raur::Handle::new();
    let cache: Vec<Update> = aur
        .info(
            old.iter()
                .map(|pkg| pkg.pkgname.to_owned())
                .collect::<Vec<_>>()
                .as_slice(),
        )
        .await
        .map_err(|_| Error::GetAurPackageFailed(None))?
        .into_iter()
        .filter_map(|new| {
            let matching_old = &old.iter().find(|old| old.pkgname == new.name)?.clone();
            let (pkgver_new, pkgrel_new) = parse_ver_and_rel(new.version).unwrap();
            Some(Update {
                pkgname: matching_old.pkgname.to_owned(),
                pkgver_cur: matching_old.pkgver.to_owned(),
                pkgrel_cur: matching_old.pkgrel.to_owned(),
                pkgver_new,
                pkgrel_new,
            })
        })
        .collect();
    Ok((
        cache
            .iter()
            .filter(|update| aur_update_due(update))
            .cloned()
            .collect(),
        cache,
    ))
}

/// Check if any AUR packages have updates to their pkgver-pkgrel.
///
/// Offline version - this function needs a reference to the latest version of
/// all aur packages (returned from `check_aur_updates_online()`.
/// # Usage
/// ```no_run
/// # async {
/// let (online, cache) = check_aur_updates_online().await.unwrap();
/// let offline = check_aur_updates_online(&cache).await.unwrap();
/// assert_eq!(online, offline);
/// // Run `paru -Syu` in the terminal
/// let offline = check_aur_updates_offline(&cache).await.unwrap();
/// assert!(offline.is_empty());
/// # };
pub async fn check_aur_updates_offline(cache: &[Update]) -> Result<Vec<Update>> {
    let old = get_aur_packages().await?;
    let updates = old
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
        .filter(aur_update_due)
        .collect();
    Ok(updates)
}

/// Returns true if a DevelUpdate is due.
fn devel_update_due(update: &DevelUpdate) -> bool {
    !update.pkgver_cur.contains(&update.ref_id_new)
}

/// Return true if an aur package is due for an update.
fn aur_update_due(package: &Update) -> bool {
    // If it's not possible to determine ordering for a package, it will be filtered
    // out. Note that this can include some VCS packages using
    // commit hashes as pkgver. That is likely acceptable behaviour
    // as VCS packages will be analyzed in check_devel_updates().
    let Some(pkgver_new) = Version::from(&package.pkgver_new) else {
        return false;
    };
    let Some(pkgver_old) = Version::from(&package.pkgver_cur) else {
        return false;
    };
    pkgver_new > pkgver_old || (pkgver_new == pkgver_old && package.pkgrel_new > package.pkgrel_cur)
}

/// pacman conf has a list of packages that should be ignored by pacman. This
/// command fetches their pkgnames.
async fn get_ignored_packages() -> Result<Vec<String>> {
    // I considered pacmanconf crate here, but it's sync, and does the same thing
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

/// Get a list of all aur packages on the system.
/// An AUR package is a package returned by `pacman -Qm` excluding ignored
/// packages.
async fn get_aur_packages() -> Result<Vec<Package>> {
    let (ignored_packages, output) = futures::join!(
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
        .map(parse_pacman_qm)
        .collect()
}

/// Get a list of all devel packages on the system.
/// A devel package is an AUR package ending with one of the `DEVEL_SUFFIXES`.
async fn get_devel_packages() -> Result<Vec<Package>> {
    let aur_packages = get_aur_packages().await?;
    Ok(aur_packages
        .into_iter()
        .filter(|package| {
            DEVEL_SUFFIXES
                .iter()
                .any(|suffix| package.pkgname.to_lowercase().contains(suffix))
        })
        .collect())
}

/// Get and parse the .SRCINFO for an aur package.
async fn get_aur_srcinfo(pkgname: &str) -> Result<Srcinfo> {
    // First we need to get the base repository from the AUR API. Since the pkgname
    // may not be the same as the repository name (and repository can contain
    // multiple packages).
    let aur = raur::Handle::new();
    let info = &aur
        .info(&[&pkgname])
        .await
        .map_err(|_| Error::GetAurPackageFailed(Some(pkgname.to_string())))?[0];
    let base = &info.package_base;

    let url = format!("https://aur.archlinux.org/cgit/aur.git/plain/.SRCINFO?h={base}");
    let raw = reqwest::get(url).await?.text().await?;
    // The pkg.pkgname field of the .SRCINO is not likely to be populated, but we'll
    // need it for later parsing, so we populate it ourself.
    let mut srcinfo = Srcinfo::from_str(&raw)?;
    srcinfo.pkg.pkgname = pkgname.to_string();

    Ok(srcinfo)
}

/// Get head identifier for a git repo - last 7 digits from commit hash.
/// If a branch is not provided, HEAD will be selected.
async fn get_head_identifier(url: String, branch: Option<&str>) -> Result<String> {
    let id = str::from_utf8(
        Command::new("git")
            .args(["ls-remote", &url, branch.unwrap_or("HEAD")])
            .output()
            .await?
            .stdout
            .as_ref(),
    )?
    .get(0..7)
    .ok_or_else(|| Error::HeadIdentifierTooShort)?
    .to_string();
    Ok(id)
}

/// Parse output of pacman -Qm into a package.
/// Example input: "watchman-bin 2024.04.15.00-1"
fn parse_pacman_qm(line: &str) -> Result<Package> {
    let (pkgname, rest) = line
        .split_once(' ')
        .ok_or_else(|| Error::ParseErrorPacman(line.to_string()))?;
    let (pkgver, pkgrel) = parse_ver_and_rel(rest)?;
    Ok(Package {
        pkgname: pkgname.to_owned(),
        pkgver,
        pkgrel,
    })
}

/// Parse output of a combined pkgrel-pkgver.
/// Example input: "1.26.15-1"
fn parse_ver_and_rel(version: impl AsRef<str>) -> Result<(String, String)> {
    let (pkgver, pkgrel) = version
        .as_ref()
        .rsplit_once('-')
        .ok_or_else(|| Error::ParseErrorPkgverPkgrel(version.as_ref().to_string()))?;
    Ok((pkgver.into(), pkgrel.into()))
}

/// Parse output line from checkupdates
/// Example input: libadwaita 1:1.6.0-1 -> 1:1.6.1-1
fn parse_update(value: &str) -> Result<Update> {
    let mut iter = value.split(' ');
    let pkgname = iter
        .next()
        .ok_or(Error::ParseErrorCheckUpdates(value.to_string()))?
        .to_string();
    let (pkgver_cur, pkgrel_cur) = parse_ver_and_rel(
        iter.next()
            .ok_or(Error::ParseErrorCheckUpdates(value.to_string()))?,
    )?;
    let (pkgver_new, pkgrel_new) = parse_ver_and_rel(
        iter.nth(1)
            .ok_or(Error::ParseErrorCheckUpdates(value.to_string()))?,
    )?;
    Ok(Update {
        pkgname,
        pkgver_cur,
        pkgrel_cur,
        pkgver_new,
        pkgrel_new,
    })
}

/// Parse source field from .SRCINFO
// NOTE: This is from paru (GPL3)
fn parse_url<'a>(source: &'a str) -> Option<PackageUrl<'a>> {
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
        check_aur_updates_offline, check_aur_updates_online, check_devel_updates_offline,
        check_devel_updates_online, check_updates, get_aur_srcinfo, get_head_identifier,
        parse_pacman_qm, parse_update, parse_url, parse_ver_and_rel, CheckType, Error, Package,
        PackageUrl, Update,
    };

    #[tokio::test]
    async fn test_check_updates() {
        let online = check_updates(CheckType::Online).await.unwrap();
        let offline = check_updates(CheckType::Offline).await.unwrap();
        assert_eq!(online, offline);
    }
    #[tokio::test]
    async fn test_check_aur_updates() {
        let (online, cache) = check_aur_updates_online().await.unwrap();
        let offline = check_aur_updates_offline(&cache).await.unwrap();
        assert_eq!(online, offline);
        eprintln!("aur {:#?}", online);
    }
    #[tokio::test]
    async fn test_check_devel_updates() {
        let (online, cache) = check_devel_updates_online().await.unwrap();
        let offline = check_devel_updates_offline(&cache).await.unwrap();
        assert_eq!(online, offline);
        eprintln!("devel {:#?}", online);
    }

    #[tokio::test]
    async fn test_get_srcinfo() {
        get_aur_srcinfo("hyprlang-git").await.unwrap();
    }
    #[tokio::test]
    async fn test_get_url() {
        let srcinfo = get_aur_srcinfo("hyprlang-git").await.unwrap();
        let url = srcinfo.base.source.first().unwrap().vec.first().unwrap();
        parse_url(url).unwrap();
    }
    #[tokio::test]
    async fn test_get_head() {
        let srcinfo = get_aur_srcinfo("hyprutils-git").await.unwrap();
        let url = srcinfo.base.source.first().unwrap().vec.first().unwrap();
        let url_parsed = parse_url(url).unwrap();
        get_head_identifier(url_parsed.remote, url_parsed.branch)
            .await
            .unwrap();
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
    fn test_parse_url_none() {
        let url = parse_url(
            "paper-icon-themegit:gopher://github.com/snwh/paper-icon-theme.git branch=main",
        );
        eprintln!("{:#?}", url);
        assert!(url.is_none());
    }
    #[test]
    fn test_parse_update() {
        let update = parse_update("libadwaita 1:1.6.0-1 -> 1:1.6.1-2").unwrap();
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
    fn test_parse_update_error() {
        let str = "libadwaita1:1.6.0-1 - 1:1.6.12";
        let update = parse_update(str).unwrap_err();
        eprintln!("{:#?}", update);
        match update {
            Error::ParseErrorCheckUpdates(s) => assert_eq!(s, str),
            _ => panic!(),
        }
    }
    #[test]
    fn test_parse_pacman_qm() {
        let update = parse_pacman_qm("winetricks-git 20240105.r47.g72b934e1-2").unwrap();
        let expected = Package {
            pkgname: "winetricks-git".to_string(),
            pkgver: "20240105.r47.g72b934e1".to_string(),
            pkgrel: "2".to_string(),
        };
        assert_eq!(update, expected);
    }
    #[test]
    fn test_parse_pacman_qm_error() {
        let str = "winetricks-git0240105.r47.g72b934e1-2";
        let update = parse_pacman_qm(str).unwrap_err();
        eprintln!("{:#?}", update);
        match update {
            Error::ParseErrorPacman(s) => assert_eq!(s, str),
            _ => panic!(),
        }
    }
    #[test]
    fn test_parse_version() {
        let actual = parse_ver_and_rel("20-240105.r47.g72b934e1-2").unwrap();
        let expected = ("20-240105.r47.g72b934e1".to_string(), "2".to_string());
        assert_eq!(actual, expected);
    }
    #[test]
    fn test_parse_version_error() {
        let str = "20240105.r47.g72b934e12";
        let actual = parse_ver_and_rel("20240105.r47.g72b934e12").unwrap_err();
        match actual {
            Error::ParseErrorPkgverPkgrel(s) => assert_eq!(s, str),
            _ => panic!(),
        }
    }
}
