//! get_updates functionality
use crate::{AurUpdate, DevelUpdate, Error, Result, DEVEL_SUFFIXES};
use core::str;
use raur::Raur;
use srcinfo::Srcinfo;
use std::str::FromStr;
use tokio::process::Command;
use version_compare::Version;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Package {
    pub pkgname: String,
    pub pkgver: String,
    pub pkgrel: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageUrl<'a> {
    pub remote: String,
    pub protocol: &'a str,
    pub branch: Option<&'a str>,
}

/// ParsedUpdate does not yet include repo name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedUpdate {
    pub pkgname: String,
    pub pkgver_cur: String,
    pub pkgrel_cur: String,
    pub pkgver_new: String,
    pub pkgrel_new: String,
}

/// Returns true if a DevelUpdate is due.
pub fn devel_update_due(update: &DevelUpdate) -> bool {
    !update.pkgver_cur.contains(&update.ref_id_new)
}

/// Return true if an aur package is due for an update.
pub fn aur_update_due(package: &AurUpdate) -> bool {
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
pub async fn get_aur_packages() -> Result<Vec<Package>> {
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
pub async fn get_devel_packages() -> Result<Vec<Package>> {
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

/// Get and parse the .SRCINFO for an package, if its in the AUR.
pub async fn get_aur_srcinfo(pkgname: &str) -> Result<Option<Srcinfo>> {
    // First we need to get the base repository from the AUR API. Since the pkgname
    // may not be the same as the repository name (and repository can contain
    // multiple packages).
    let aur = raur::Handle::new();
    let Some(ref info) = aur
        .info(&[&pkgname])
        .await
        .map_err(|_| Error::GetAurPackageFailed(Some(pkgname.to_string())))?
        .into_iter()
        .next()
    else {
        return Ok(None);
    };
    let base = &info.package_base;

    let url = format!("https://aur.archlinux.org/cgit/aur.git/plain/.SRCINFO?h={base}");
    let raw = reqwest::get(url).await?.text().await?;
    // The pkg.pkgname field of the .SRCINO is not likely to be populated, but we'll
    // need it for later parsing, so we populate it ourself.
    let mut srcinfo = Srcinfo::from_str(&raw)?;
    srcinfo.pkg.pkgname = pkgname.to_string();

    Ok(Some(srcinfo))
}

/// Get head identifier for a git repo - last 7 digits from commit hash.
/// If a branch is not provided, HEAD will be selected.
pub async fn get_head_identifier(url: String, branch: Option<&str>) -> Result<String> {
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
pub fn parse_ver_and_rel(version: impl AsRef<str>) -> Result<(String, String)> {
    let (pkgver, pkgrel) = version
        .as_ref()
        .rsplit_once('-')
        .ok_or_else(|| Error::ParseErrorPkgverPkgrel(version.as_ref().to_string()))?;
    Ok((pkgver.into(), pkgrel.into()))
}

/// Parse output line from checkupdates
/// Example input: libadwaita 1:1.6.0-1 -> 1:1.6.1-1
pub fn parse_update(value: &str) -> Result<ParsedUpdate> {
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
    Ok(ParsedUpdate {
        pkgname,
        pkgver_cur,
        pkgrel_cur,
        pkgver_new,
        pkgrel_new,
    })
}

/// Parse source field from .SRCINFO
// NOTE: This is from paru (GPL3)
pub fn parse_url(source: &str) -> Option<PackageUrl> {
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
    use super::*;

    #[tokio::test]
    async fn test_get_srcinfo() {
        get_aur_srcinfo("hyprlang-git").await.unwrap();
    }
    #[tokio::test]
    async fn test_get_srcinfo_not_in_aur() {
        let output = get_aur_srcinfo("made-up-package").await.unwrap();
        assert!(output.is_none());
    }
    #[tokio::test]
    async fn test_get_url() {
        let srcinfo = get_aur_srcinfo("hyprlang-git").await.unwrap().unwrap();
        let url = srcinfo.base.source.first().unwrap().vec.first().unwrap();
        parse_url(url).unwrap();
    }
    #[tokio::test]
    async fn test_get_head() {
        let srcinfo = get_aur_srcinfo("hyprutils-git").await.unwrap().unwrap();
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
        let expected = ParsedUpdate {
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
