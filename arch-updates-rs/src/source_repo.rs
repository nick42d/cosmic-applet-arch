//! Get the source repo of a package.

use super::Result;
use core::str;
use std::{fmt::Display, io::BufRead};

/// Source of a package.
// https://wiki.archlinux.org/title/Official_repositories
pub enum SourceRepo {
    Core,
    Extra,
    Multilib,
    CoreTesting,
    ExtraTesting,
    MultilibTesting,
    GnomeUnstable,
    KdeUnstable,
    /// Packages not in pacman local database.
    /// This includes manually installed PKGBUILDs, or AUR PKGBUILDs.
    Foreign,
    /// Packages in an unofficial user repository
    /// e.g. https://wiki.archlinux.org/title/Unofficial_user_repositories or endeavouros/manjaro repositories.
    Other(String),
}

async fn get_package_repo(pkgname: String) -> Result<SourceRepo> {
    Ok(SourceRepo::from_text(
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
        .find(|line| line.contains(&format!(" {pkgname} ")))
        .unwrap()
        .split_once(' ')
        .unwrap()
        .0,
    ))
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
            other => Self::Other(text.to_string()),
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
            SourceRepo::Foreign => write!(f, "foreign"),
            SourceRepo::Other(s) => write!(f, "{s}"),
        }
    }
}
