//! Get the source repo of a package.

use super::Result;
use crate::{get_updates::ParsedUpdate, Update};
use core::str;
use std::{collections::HashMap, fmt::Display};

/// Maps pkgnames to their source repos.
pub type SourcesList = HashMap<String, SourceRepo>;

/// Source of a package.
/// https://wiki.archlinux.org/title/Official_repositories
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceRepo {
    Core,
    Extra,
    Multilib,
    CoreTesting,
    ExtraTesting,
    MultilibTesting,
    GnomeUnstable,
    KdeUnstable,
    /// Aur is not strictly a source repo, but if we know the package has been
    /// isntalled from an Aur PKGBUILD, this can be a useful attribute.
    Aur,
    /// Other comprises packages in an unofficial user repository
    /// e.g. https://wiki.archlinux.org/title/Unofficial_user_repositories or endeavouros/manjaro repositories.
    Other(String),
}

pub fn merge_source_info(updates: Vec<ParsedUpdate>, source_info: &SourcesList) -> Vec<Update> {
    updates
        .into_iter()
        .map(|update| {
            let ParsedUpdate {
                pkgname,
                pkgver_cur,
                pkgrel_cur,
                pkgver_new,
                pkgrel_new,
            } = update;
            Update {
                source_repo: source_info.get(&pkgname).map(|r| r.to_owned()),
                pkgname,
                pkgver_cur,
                pkgrel_cur,
                pkgver_new,
                pkgrel_new,
            }
        })
        .collect()
}

/// Using `pacman -Sl`, get a list of all packages in the local sync db and
/// their source repos.
pub async fn get_sources_list() -> Result<SourcesList> {
    Ok(str::from_utf8(
        // pacman -Sl lists all packages in sync db and output is like:
        // `{repo} {pkgname} {pkgver}-{pkgrel} ?[installed]`
        tokio::process::Command::new("pacman")
            .arg("-Sl")
            .output()
            .await?
            .stdout
            .as_slice(),
    )?
    .lines()
    //TODO: error handling
    .map(|pkgline| pkgline.split_once(' ').unwrap().0)
    .map(|pkgname| (pkgname.to_string(), SourceRepo::from_text(pkgname)))
    .collect())
}

struct PackageSource {
    pkgname: String,
    source_name: String,
}

fn parse_pacman_sl_line(line: &str) -> Result<PackageSource> {}

impl SourceRepo {
    fn from_text(text: &str) -> Self {
        match text {
            "core" => Self::Core,
            "extra" => Self::Extra,
            "multilib" => Self::Multilib,
            "core-testing" => Self::CoreTesting,
            "extra-testing" => Self::ExtraTesting,
            "multilib-testing" => Self::MultilibTesting,
            "gnome-unstable" => Self::GnomeUnstable,
            "kde-unstable" => Self::KdeUnstable,
            other => Self::Other(other.to_string()),
        }
    }
}

impl Display for SourceRepo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceRepo::Core => write!(f, "core"),
            SourceRepo::Extra => write!(f, "extra"),
            SourceRepo::Multilib => write!(f, "multilib"),
            SourceRepo::CoreTesting => write!(f, "core-testing"),
            SourceRepo::ExtraTesting => write!(f, "extra-testing"),
            SourceRepo::MultilibTesting => write!(f, "multilib-testing"),
            SourceRepo::GnomeUnstable => write!(f, "gnome-unstable"),
            SourceRepo::KdeUnstable => write!(f, "kde-unstable"),
            SourceRepo::Aur => write!(f, "aur"),
            SourceRepo::Other(s) => write!(f, "{s}"),
        }
    }
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
