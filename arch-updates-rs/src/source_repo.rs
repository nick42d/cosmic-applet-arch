//! Get the source repo of a package.

use super::Result;
use crate::get_updates::ParsedUpdate;
use crate::{Error, PacmanUpdate};
use core::str;
use std::collections::HashMap;
use std::fmt::Display;

/// Maps pkgnames to their source repos.
pub type SourcesList = HashMap<String, SourceRepo>;

/// Source of a package.
/// <https://wiki.archlinux.org/title/Official_repositories>
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
    /// Other comprises packages in an unofficial user repository
    /// e.g. <https://wiki.archlinux.org/title/Unofficial_user_repositories> or endeavouros/manjaro repositories.
    Other(String),
}

/// When given a list of ParsedUpdates, transform them into Updates using
/// reference data sources_list.
/// Returns an error if ParsedUpdate is not in SourcesList.
pub fn add_sources_to_updates(
    updates: Vec<ParsedUpdate>,
    sources_list: &SourcesList,
) -> Vec<PacmanUpdate> {
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
            PacmanUpdate {
                source_repo: sources_list.get(&pkgname).map(|r| r.to_owned()),
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
    str::from_utf8(
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
    .map(parse_pacman_sl)
    .collect::<Result<_>>()
}

/// Parse output of pacman -Sl into a PackageSource.
/// Example input: "core pacman 7.0.0.r6.gc685ae6-1 [installed]"
/// Returns Result<(pkgname, source_repo)>
fn parse_pacman_sl(line: &str) -> Result<(String, SourceRepo)> {
    let mut parts = line.split(' ');
    let source_repo_name = parts
        .next()
        .ok_or_else(|| Error::ParseErrorPacman(line.to_string()))?;
    let pkgname = parts
        .next()
        .ok_or_else(|| Error::ParseErrorPacman(line.to_string()))?
        .to_string();
    Ok((pkgname, SourceRepo::from_text(source_repo_name)))
}

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
            SourceRepo::Other(s) => write!(f, "{s}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::get_updates::ParsedUpdate;
    use crate::source_repo::{
        add_sources_to_updates, get_sources_list, parse_pacman_sl, SourceRepo,
    };
    use crate::{Error, PacmanUpdate};

    #[tokio::test]
    async fn test_get_sources_list() {
        let sources_list = get_sources_list().await;
        assert!(sources_list.is_ok())
    }

    #[test]
    fn test_parse_pacman_sl() {
        let (pkgname, source_repo) =
            parse_pacman_sl("core pacman 7.0.0.r6.gc685ae6-1 [installed]").unwrap();
        assert_eq!(pkgname, "pacman".to_string());
        assert_eq!(source_repo, SourceRepo::Core);
    }
    #[test]
    fn test_parse_pacman_sl_error() {
        let str = "pacman-7.0.0.r6.gc685ae6-1[installed]";
        let err = parse_pacman_sl(str).unwrap_err();
        match err {
            Error::ParseErrorPacman(s) => assert_eq!(s, str),
            _ => panic!(),
        }
    }
    #[test]
    fn test_add_sources_to_updates() {
        let parsed_sources = vec![
            ParsedUpdate {
                pkgname: "pacman".to_string(),
                pkgver_cur: "1.0.0".to_string(),
                pkgrel_cur: "1".to_string(),
                pkgver_new: "1.1.1".to_string(),
                pkgrel_new: "1".to_string(),
            },
            ParsedUpdate {
                pkgname: "linux-aur".to_string(),
                pkgver_cur: "6.12.1.aur1".to_string(),
                pkgrel_cur: "1".to_string(),
                pkgver_new: "6.13.1.aur1".to_string(),
                pkgrel_new: "1".to_string(),
            },
        ];
        let sources_list = [
            ("linux-zen".to_string(), SourceRepo::Extra),
            ("linux".to_string(), SourceRepo::Core),
            ("pacman".to_string(), SourceRepo::Core),
        ]
        .into();
        let expected = vec![
            PacmanUpdate {
                pkgname: "pacman".to_string(),
                pkgver_cur: "1.0.0".to_string(),
                pkgrel_cur: "1".to_string(),
                pkgver_new: "1.1.1".to_string(),
                pkgrel_new: "1".to_string(),
                source_repo: Some(SourceRepo::Core),
            },
            PacmanUpdate {
                pkgname: "linux-aur".to_string(),
                pkgver_cur: "6.12.1.aur1".to_string(),
                pkgrel_cur: "1".to_string(),
                pkgver_new: "6.13.1.aur1".to_string(),
                pkgrel_new: "1".to_string(),
                source_repo: None,
            },
        ];
        let merged = add_sources_to_updates(parsed_sources, &sources_list);
        assert_eq!(expected, merged);
    }
}
