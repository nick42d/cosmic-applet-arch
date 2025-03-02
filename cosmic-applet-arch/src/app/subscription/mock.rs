use crate::news::{DatedNewsItem, WarnedResult};
use arch_updates_rs::{AurUpdate, DevelUpdate, PacmanUpdate, SourceRepo};
use chrono::FixedOffset;
use serde::Deserialize;

use super::core::Updates;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct MockDatedNewsItem {
    pub title: Option<String>,
    pub link: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub date: chrono::DateTime<FixedOffset>,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub enum MockSourceRepo {
    Core,
    Extra,
    Multilib,
    CoreTesting,
    ExtraTesting,
    MultilibTesting,
    GnomeUnstable,
    KdeUnstable,
    Other(String),
}
#[derive(Clone, Debug, Default, Deserialize)]
pub struct MockUpdates {
    pub pacman: Vec<MockPacmanUpdate>,
    pub aur: Vec<MockAurUpdate>,
    pub devel: Vec<MockDevelUpdate>,
}
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct MockPacmanUpdate {
    pub pkgname: String,
    pub pkgver_cur: String,
    pub pkgrel_cur: String,
    pub pkgver_new: String,
    pub pkgrel_new: String,
    pub source_repo: Option<MockSourceRepo>,
}
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct MockAurUpdate {
    pub pkgname: String,
    pub pkgver_cur: String,
    pub pkgrel_cur: String,
    pub pkgver_new: String,
    pub pkgrel_new: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct MockDevelUpdate {
    pub pkgname: String,
    pub pkgver_cur: String,
    pub pkgrel_cur: String,
    pub ref_id_new: String,
}
impl From<MockDatedNewsItem> for DatedNewsItem {
    fn from(value: MockDatedNewsItem) -> Self {
        let MockDatedNewsItem {
            title,
            link,
            description,
            author,
            date,
        } = value;
        DatedNewsItem {
            title,
            link,
            description,
            author,
            date,
        }
    }
}
impl From<MockUpdates> for Updates {
    fn from(value: MockUpdates) -> Updates {
        let MockUpdates { pacman, aur, devel } = value;
        Updates {
            pacman: pacman.into_iter().map(Into::into).collect(),
            aur: aur.into_iter().map(Into::into).collect(),
            devel: devel.into_iter().map(Into::into).collect(),
        }
    }
}
impl From<MockDevelUpdate> for DevelUpdate {
    fn from(value: MockDevelUpdate) -> DevelUpdate {
        let MockDevelUpdate {
            pkgname,
            pkgver_cur,
            pkgrel_cur,
            ref_id_new,
        } = value;
        DevelUpdate {
            pkgname,
            pkgver_cur,
            pkgrel_cur,
            ref_id_new,
        }
    }
}
impl From<MockPacmanUpdate> for PacmanUpdate {
    fn from(value: MockPacmanUpdate) -> PacmanUpdate {
        let MockPacmanUpdate {
            pkgname,
            pkgver_cur,
            pkgrel_cur,
            pkgver_new,
            pkgrel_new,
            source_repo,
        } = value;
        PacmanUpdate {
            pkgname,
            pkgver_cur,
            pkgrel_cur,
            pkgver_new,
            pkgrel_new,
            source_repo: source_repo.map(Into::into),
        }
    }
}
impl From<MockAurUpdate> for AurUpdate {
    fn from(value: MockAurUpdate) -> AurUpdate {
        let MockAurUpdate {
            pkgname,
            pkgver_cur,
            pkgrel_cur,
            pkgver_new,
            pkgrel_new,
        } = value;
        AurUpdate {
            pkgname,
            pkgver_cur,
            pkgrel_cur,
            pkgver_new,
            pkgrel_new,
        }
    }
}
impl From<MockSourceRepo> for SourceRepo {
    fn from(value: MockSourceRepo) -> SourceRepo {
        match value {
            MockSourceRepo::Core => SourceRepo::Core,
            MockSourceRepo::Extra => SourceRepo::Extra,
            MockSourceRepo::Multilib => SourceRepo::Multilib,
            MockSourceRepo::CoreTesting => SourceRepo::CoreTesting,
            MockSourceRepo::ExtraTesting => SourceRepo::ExtraTesting,
            MockSourceRepo::MultilibTesting => SourceRepo::MultilibTesting,
            MockSourceRepo::GnomeUnstable => SourceRepo::GnomeUnstable,
            MockSourceRepo::KdeUnstable => SourceRepo::KdeUnstable,
            MockSourceRepo::Other(other) => SourceRepo::Other(other),
        }
    }
}
pub async fn get_mock_updates() -> arch_updates_rs::Result<Updates> {
    let file = tokio::fs::read_to_string("test/mock_updates.ron")
        .await
        .unwrap();
    let updates: MockUpdates = ron::from_str(&file).unwrap();
    Ok(updates.into())
}
pub async fn get_mock_news() -> WarnedResult<Vec<DatedNewsItem>, String, anyhow::Error> {
    let file = tokio::fs::read_to_string("test/mock_news.ron")
        .await
        .unwrap();
    let mock_news: Vec<MockDatedNewsItem> = ron::from_str(&file).unwrap();
    let news = mock_news.into_iter().map(Into::into).collect();
    WarnedResult::Ok(news)
}
