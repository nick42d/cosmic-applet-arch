//! # arch_updates_rs
//! Library to query arch linux packaging tools to see if updates are available.
//! Designed for cosmic-applet-arch, but could be used in similar apps as well.
//!
//! # Usage example
//! This example shows how to check for updates online and print them to the
//! terminal. It also shows how to check for updates offline, using the cache
//! returned from the online check. If a system update is run in between as per
//! the example, the offline check should return 0 updates due.
//!
//!```no_run
//! use arch_updates_rs::*;
//!
//! #[tokio::main]
//! pub async fn main() {
//!     let (Ok((pacman, pacman_cache)), Ok((aur, aur_cache)), Ok((devel, devel_cache))) = tokio::join!(
//!         check_pacman_updates_online(),
//!         check_aur_updates_online(),
//!         check_devel_updates_online(),
//!     ) else {
//!         panic!();
//!     };
//!     println!("pacman: {:#?}", pacman);
//!     println!("aur: {:#?}", aur);
//!     println!("devel: {:#?}", devel);
//!     std::process::Command::new("paru")
//!         .arg("-Syu")
//!         .spawn()
//!         .unwrap()
//!         .wait()
//!         .unwrap();
//!     let (Ok(pacman), Ok(aur), Ok(devel)) = tokio::join!(
//!         check_pacman_updates_offline(&pacman_cache),
//!         check_aur_updates_offline(&aur_cache),
//!         check_devel_updates_offline(&devel_cache),
//!     ) else {
//!         panic!();
//!     };
//!     assert!(pacman.is_empty() && aur.is_empty() && devel.is_empty());
//! }
//! ```
use core::str;
use futures::{future::try_join, stream::FuturesOrdered, StreamExt, TryStreamExt};
use get_updates::{
    aur_update_due, checkupdates, devel_update_due, get_aur_packages, get_aur_srcinfo,
    get_devel_packages, get_head_identifier, parse_update, parse_url, parse_ver_and_rel,
    CheckupdatesMode, PackageUrl,
};
use raur::Raur;
use source_repo::{add_sources_to_updates, get_sources_list, SourcesList};
use std::{io, str::Utf8Error};
use thiserror::Error;

mod get_updates;
mod source_repo;

pub use source_repo::SourceRepo;

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
    #[error("checkupdates returned an error: `{0}`")]
    CheckUpdatesReturnedError(String),
    #[error("Failed to parse update from checkupdates string: `{0}`")]
    ParseErrorCheckUpdates(String),
    #[error("Failed to parse update from pacman string: `{0}`")]
    ParseErrorPacman(String),
    #[error("Failed to parse pkgver and pkgrel from string `{0}`")]
    ParseErrorPkgverPkgrel(String),
}

/// Current status of an installed pacman package, vs the status of the latest
/// version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PacmanUpdate {
    pub pkgname: String,
    pub pkgver_cur: String,
    pub pkgrel_cur: String,
    pub pkgver_new: String,
    pub pkgrel_new: String,
    pub source_repo: Option<SourceRepo>,
}

/// Current status of an installed AUR package, vs the status of the latest
/// version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AurUpdate {
    pub pkgname: String,
    pub pkgver_cur: String,
    pub pkgrel_cur: String,
    pub pkgver_new: String,
    pub pkgrel_new: String,
}

/// Current status of an installed devel package, vs latest commit hash on the
/// source repo.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevelUpdate {
    pub pkgname: String,
    pub pkgver_cur: String,
    pub pkgrel_cur: String,
    /// When checking a devel update, we don't get a pkgver/pkgrel so-to-speak,
    /// we instead get the github ref.
    pub ref_id_new: String,
}

/// Cached state for offline updates check
#[derive(Default, Clone)]
pub struct PacmanUpdatesCache(SourcesList);
/// Cached state for offline updates check
#[derive(Default, Clone)]
pub struct AurUpdatesCache(Vec<AurUpdate>);
#[derive(Default, Clone)]
/// Cached state for offline updates check
pub struct DevelUpdatesCache(Vec<DevelUpdate>);

/// Use the `checkupdates` function to check if any pacman-managed packages have
/// updates due.
///
/// Online version - this function uses the network.
/// Returns a tuple of:
///  - Packages that are not up to date.
///  - Cache that can be stored in memory to make next query more efficient.
///
/// # Note
/// This will fail with an error if somebody else is running 'checkupdates' in sync
/// mode at the same time.
/// # Usage
/// ```no_run
/// # use arch_updates_rs::*;
/// # async {
/// let updates = check_pacman_updates_online().await.unwrap();
/// // Run `sudo pacman -Syu` in the terminal
/// let (updates, _) = check_pacman_updates_online().await.unwrap();
/// assert!(updates.is_empty());
/// # };
pub async fn check_pacman_updates_online() -> Result<(Vec<PacmanUpdate>, PacmanUpdatesCache)> {
    let (parsed_updates, source_info) =
        try_join(checkupdates(CheckupdatesMode::Sync), get_sources_list()).await?;
    let updates = add_sources_to_updates(parsed_updates, &source_info);
    Ok((updates, PacmanUpdatesCache(source_info)))
}

/// Use the `checkupdates` function to check if any pacman-managed packages have
/// updates due.
///
/// Offline version - this function doesn't use the network, it takes the cache
/// returned from `check_pacman_updates_online()` to avoid too many queries to
/// pacman's sync db.
///
///
/// # Usage
/// ```no_run
/// # use arch_updates_rs::*;
/// # async {
/// let (online, cache) = check_pacman_updates_online().await.unwrap();
/// let offline = check_pacman_updates_offline(&cache).await.unwrap();
/// assert_eq!(online, offline);
/// // Run `sudo pacman -Syu` in the terminal
/// let offline = check_pacman_updates_offline(&cache).await.unwrap();
/// assert!(offline.is_empty());
/// # };
pub async fn check_pacman_updates_offline(cache: &PacmanUpdatesCache) -> Result<Vec<PacmanUpdate>> {
    let parsed_updates = checkupdates(CheckupdatesMode::NoSync).await?;
    Ok(add_sources_to_updates(parsed_updates, &cache.0))
}

/// Check if any packages ending in `DEVEL_SUFFIXES` have updates to their
/// source repositories.
///
/// Online version - this function checks the network.
/// Returns a tuple of:
///  - Packages that are not up to date.
///  - Cache for offline use.
///
/// # Notes
///  - For this to be accurate, it's reliant on each devel package having only
///    one source URL. If this is not the case, the function will produce a
///    DevelUpdate for each source url, and may assume one or more are out of
///    date.
///  - This is also reliant on VCS packages being good
///    citizens and following the VCS Packaging Guidelines.
///    <https://wiki.archlinux.org/title/VCS_package_guidelines>
/// # Usage
/// ```no_run
/// # use arch_updates_rs::*;
/// # async {
/// let (updates, _) = check_devel_updates_online().await.unwrap();
/// // Run `paru -Syu` in the terminal
/// let (updates, _) = check_devel_updates_online().await.unwrap();
/// assert!(updates.is_empty());
/// # };
pub async fn check_devel_updates_online() -> Result<(Vec<DevelUpdate>, DevelUpdatesCache)> {
    let devel_packages = get_devel_packages().await?;
    let devel_updates = futures::stream::iter(devel_packages.into_iter())
        // Get the SRCINFO for each package (as Result<Option<_>>).
        .then(|pkg| async move {
            let srcinfo = get_aur_srcinfo(&pkg.pkgname).await;
            (pkg, srcinfo)
        })
        // Remove any None values from the list - these are where the aurweb
        // api call was succesful but the package wasn't found (ie, package is not an AUR package).
        .filter_map(|(pkg, maybe_srcinfo)| async { Some((pkg, maybe_srcinfo.transpose()?)) })
        .then(|(pkg, srcinfo)| async move {
            let updates = srcinfo?
                .base
                .source
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
                .collect::<FuturesOrdered<_>>();
            Ok::<_, Error>(updates)
        })
        .try_flatten()
        .try_collect::<Vec<_>>()
        .await?;
    Ok((
        devel_updates
            .iter()
            .filter(|update| devel_update_due(update))
            .cloned()
            .collect::<Vec<_>>(),
        DevelUpdatesCache(devel_updates),
    ))
}

/// Check if any packages ending in `DEVEL_SUFFIXES` have updates to their
/// source repositories.
///
/// Offline version - this function takes the cache returned from
/// `check_devel_updates_online()`.
///
/// # Usage
/// ```no_run
/// # use arch_updates_rs::*;
/// # async {
/// let (online, cache) = check_devel_updates_online().await.unwrap();
/// let offline = check_devel_updates_offline(&cache).await.unwrap();
/// assert_eq!(online, offline);
/// // Run `paru -Syu` in the terminal
/// let offline = check_devel_updates_offline(&cache).await.unwrap();
/// assert!(offline.is_empty());
/// # };
pub async fn check_devel_updates_offline(cache: &DevelUpdatesCache) -> Result<Vec<DevelUpdate>> {
    let devel_packages = get_devel_packages().await?;
    let devel_updates = devel_packages
        .iter()
        .flat_map(|package| {
            cache
                .0
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
/// # use arch_updates_rs::*;
/// # async {
/// let (updates, _) = check_aur_updates_online().await.unwrap();
/// // Run `paru -Syu` in the terminal
/// let (updates, _) = check_aur_updates_online().await.unwrap();
/// assert!(updates.is_empty());
/// # };
pub async fn check_aur_updates_online() -> Result<(Vec<AurUpdate>, AurUpdatesCache)> {
    let old = get_aur_packages().await?;
    let aur = raur::Handle::new();
    let cache: Vec<AurUpdate> = aur
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
            let matching_old = old.iter().find(|old| old.pkgname == new.name)?.clone();
            let maybe_old_ver_and_rel = parse_ver_and_rel(new.version);
            Some((matching_old, maybe_old_ver_and_rel))
        })
        .map(|(matching_old, maybe_old_ver_and_rel)| -> Result<_> {
            let (pkgver_new, pkgrel_new) = maybe_old_ver_and_rel?;
            Ok(AurUpdate {
                pkgname: matching_old.pkgname.to_owned(),
                pkgver_cur: matching_old.pkgver.to_owned(),
                pkgrel_cur: matching_old.pkgrel.to_owned(),
                pkgver_new,
                pkgrel_new,
            })
        })
        .collect::<Result<_>>()?;
    Ok((
        cache
            .iter()
            .filter(|update| aur_update_due(update))
            .cloned()
            .collect(),
        AurUpdatesCache(cache),
    ))
}

/// Check if any AUR packages have updates to their pkgver-pkgrel.
///
/// Offline version - this function needs a reference to the latest version of
/// all aur packages (returned from `check_aur_updates_online()`.
///
/// # Usage
/// ```no_run
/// # use arch_updates_rs::*;
/// # async {
/// let (online, cache) = check_aur_updates_online().await.unwrap();
/// let offline = check_aur_updates_offline(&cache).await.unwrap();
/// assert_eq!(online, offline);
/// // Run `paru -Syu` in the terminal
/// let offline = check_aur_updates_offline(&cache).await.unwrap();
/// assert!(offline.is_empty());
/// # };
pub async fn check_aur_updates_offline(cache: &AurUpdatesCache) -> Result<Vec<AurUpdate>> {
    let old = get_aur_packages().await?;
    let updates = old
        .iter()
        .map(|old_package| {
            let matching_cached = cache
                .0
                .iter()
                .find(|cache_package| cache_package.pkgname == old_package.pkgname);
            let (pkgver_new, pkgrel_new) = match matching_cached {
                Some(cache_package) => (
                    cache_package.pkgver_new.to_owned(),
                    cache_package.pkgrel_new.to_owned(),
                ),
                None => (old_package.pkgver.to_owned(), old_package.pkgrel.to_owned()),
            };
            AurUpdate {
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

#[cfg(test)]
mod tests {
    use crate::{
        check_aur_updates_offline, check_aur_updates_online, check_devel_updates_offline,
        check_devel_updates_online, check_pacman_updates_offline, check_pacman_updates_online,
    };

    #[tokio::test]
    async fn test_check_pacman_updates() {
        let (online, cache) = check_pacman_updates_online().await.unwrap();
        let offline = check_pacman_updates_offline(&cache).await.unwrap();
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
}
