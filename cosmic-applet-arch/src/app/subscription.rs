use super::{CosmicAppletArch, Message, CYCLES, SUBSCRIPTION_BUF_SIZE};
use crate::app::{INTERVAL, TIMEOUT};
use crate::news::NewsCache;
use crate::news::{DatedNewsItem, WarnedResult};
use arch_updates_rs::{
    AurUpdate, AurUpdatesCache, DevelUpdate, DevelUpdatesCache, PacmanUpdate, PacmanUpdatesCache,
};
use chrono::{DateTime, Local};
use cosmic::iced::futures::{channel::mpsc, SinkExt};
use futures::{FutureExt, TryFutureExt};
use std::future::Future;
use tokio::join;

// Long running stream of messages to the app.
pub fn subscription(app: &CosmicAppletArch) -> cosmic::iced::Subscription<Message> {
    let refresh_pressed_notifier = app.refresh_pressed_notifier.clone();
    let clear_news_pressed_notifier = app.clear_news_pressed_notifier.clone();
    async fn send_update_error(tx: &mut mpsc::Sender<Message>, e: impl std::fmt::Display) {
        tx.send(Message::CheckUpdatesErrorsMsg(format!("{e}")))
            .await
            .unwrap_or_else(|e| {
                eprintln!(
                    "Error {e} sending Arch update status - maybe the applet has been dropped."
                )
            });
    }
    async fn send_update(
        tx: &mut mpsc::Sender<Message>,
        updates: Updates,
        checked_online_time: Option<DateTime<Local>>,
    ) {
        tx.send(Message::CheckUpdatesMsg {
            updates,
            checked_online_time,
        })
        .await
        .unwrap_or_else(|e| {
            eprintln!("Error {e} sending Arch update status - maybe the applet has been dropped.")
        });
    }
    async fn send_news(tx: &mut mpsc::Sender<Message>, news: Vec<DatedNewsItem>) {
        tx.send(Message::CheckNewsMsg(news))
            .await
            .unwrap_or_else(|e| {
                eprintln!("Error {e} sending Arch news status - maybe the applet has been dropped.")
            });
    }
    async fn send_news_error(tx: &mut mpsc::Sender<Message>, e: impl std::fmt::Display) {
        tx.send(Message::CheckNewsErrorsMsg(format!("{e}")))
            .await
            .unwrap_or_else(|e| {
                eprintln!("Error {e} sending Arch news status - maybe the applet has been dropped.")
            });
    }
    // TODO: Determine if INTERVAL is sufficient to prevent too many timeouts.
    let updates_worker = |mut tx: mpsc::Sender<Message>| async move {
        let mut counter = 0;
        // If we have no cache, that means we haven't run a succesful online check.
        // Offline checks will be skipped until we can run one.
        let mut cache = None;
        let mut interval = tokio::time::interval(INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            let notified = refresh_pressed_notifier.notified();
            tokio::select! {
                _ = interval.tick() => {
                    let check_type = match counter {
                        0 => CheckType::Online,
                        _ => CheckType::Offline,
                    };
                    counter += 1;
                    if counter > CYCLES {
                        counter = 0
                    }
                    let updates = match (&check_type, &cache) {
                        (CheckType::Online, _) => {
                            match flat_erased_timeout(TIMEOUT, get_updates_online()).await {
                                Err(e) => {
                                    cache = None;
                                    send_update_error(&mut tx, e).await;
                                    continue;
                                },
                                Ok((updates, cache_tmp)) => {
                                    cache = Some(cache_tmp);
                                    updates
                                }
                            }
                        }
                        (CheckType::Offline, Some(cache)) => {
                            match flat_erased_timeout(TIMEOUT, get_updates_offline(cache)).await {
                                Err(e) => {
                                    send_update_error(&mut tx, e).await;
                                    continue;
                                },
                                Ok(updates) => updates
                            }
                        }
                        (CheckType::Offline, None) => continue,
                    };
                    let checked_online_time = match check_type {
                        CheckType::Online => Some(Local::now()),
                        CheckType::Offline => None,
                    };
                    send_update(&mut tx, updates, checked_online_time).await;
                }
                _ = notified => {
                    counter = 1;
                    let updates = flat_erased_timeout(TIMEOUT, get_updates_online()).await;
                    match updates {
                        Ok((updates, cache_tmp)) => {
                            cache = Some(cache_tmp);
                            send_update(&mut tx, updates, Some(Local::now())).await;
                        },
                        Err(e) => {
                            cache = None;
                            send_update_error(&mut tx, e).await;
                        }
                    }
                }
            }
        }
    };
    let news_worker = |mut tx: mpsc::Sender<Message>| async move {
        let mut counter = 0;
        // If we have no cache, that means we haven't run a succesful online check.
        // Offline checks will be skipped until we can run one.
        let mut cache = None;
        let mut interval = tokio::time::interval(INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            let notified = clear_news_pressed_notifier.notified();
            tokio::select! {
                _ = interval.tick() => {
                    let check_type = match counter {
                        0 => CheckType::Online,
                        _ => CheckType::Offline,
                    };
                    counter += 1;
                    if counter > CYCLES {
                        counter = 0
                    }
                    let updates = match (&check_type, &cache) {
                        (CheckType::Online, _) => {
                            match flat_erased_timeout(TIMEOUT, get_news_online().map(consume_warning)).await {
                                Err(e) => {
                                    cache = None;
                                    send_news_error(&mut tx, e).await;
                                    continue;
                                },
                                Ok((updates, cache_tmp)) => {
                                    cache = Some(cache_tmp);
                                    updates
                                }
                            }
                        }
                        (CheckType::Offline, Some(cache)) => {
                            match flat_erased_timeout(TIMEOUT, get_news_offline(cache).map(consume_warning)).await {
                                Err(e) => {
                                    send_news_error(&mut tx, e).await;
                                    continue;
                                },
                                Ok(updates) => updates
                            }
                        }
                        (CheckType::Offline, None) => continue,
                    };
                    let checked_online_time = match check_type {
                        CheckType::Online => Some(Local::now()),
                        CheckType::Offline => None,
                    };
                    send_news(&mut tx, updates).await;
                }
                _ = notified => {
                    counter = 1;
                    todo!();
                    // let updates = flat_erased_timeout(TIMEOUT,todo!()).await;
                    // match updates {
                    //     Ok((updates, cache_tmp)) => {
                    //         cache = Some(cache_tmp);
                    //         send_update(&mut tx, updates, Some(Local::now())).await;
                    //     },
                    //     Err(e) => {
                    //         cache = None;
                    //         send_update_error(&mut tx, e).await;
                    //     }
                    // }
                }
            }
        }
    };
    let updates_stream =
        cosmic::iced_futures::stream::channel(SUBSCRIPTION_BUF_SIZE, updates_worker);
    let news_stream = cosmic::iced_futures::stream::channel(SUBSCRIPTION_BUF_SIZE, news_worker);
    let updates_sub = cosmic::iced::Subscription::run_with_id("arch-updates-sub", updates_stream);
    let news_sub = cosmic::iced::Subscription::run_with_id("arch-news-sub", news_stream);
    cosmic::iced::Subscription::batch([updates_sub, news_sub])
}

#[derive(Clone, Copy, Debug)]
enum CheckType {
    Online,
    Offline,
}

#[derive(Default, Clone)]
struct CacheState {
    pacman_cache: PacmanUpdatesCache,
    aur_cache: AurUpdatesCache,
    devel_cache: DevelUpdatesCache,
}

#[derive(Clone, Debug, Default)]
pub struct Updates {
    pub pacman: Vec<PacmanUpdate>,
    pub aur: Vec<AurUpdate>,
    pub devel: Vec<DevelUpdate>,
}

/// Helper function - adds a timeout to a future that returns a result.
/// Type erases the error by converting to string, avoiding nested results.
async fn flat_erased_timeout<T, E, Fut>(duration: std::time::Duration, f: Fut) -> Result<T, String>
where
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let res = tokio::time::timeout(duration, f.map_err(|e| format!("{e}")))
        .map_err(|_| "API call timed out".to_string())
        .await;
    match res {
        Ok(Err(e)) | Err(e) => Err(e),
        Ok(Ok(t)) => Ok(t),
    }
}

/// Turn a WarnedResult into a Result, emitting an effect if a warning existed (print to stderr).
fn consume_warning<T, W: std::fmt::Display, E>(w: WarnedResult<T, W, E>) -> Result<T, E> {
    match w {
        WarnedResult::Ok(t) => Ok(t),
        WarnedResult::Warning(t, w) => {
            eprintln!("Warning: {w}");
            Ok(t)
        }
        WarnedResult::Err(e) => Err(e),
    }
}

#[cfg(feature = "mock-api")]
async fn get_news_offline(
    _: &NewsCache,
) -> WarnedResult<Vec<DatedNewsItem>, String, anyhow::Error> {
    todo!()
}

#[cfg(not(feature = "mock-api"))]
async fn get_news_offline(
    cache: &NewsCache,
) -> WarnedResult<Vec<DatedNewsItem>, String, anyhow::Error> {
    crate::news::get_news_offline(&cache).await
}

#[cfg(feature = "mock-api")]
async fn get_news_online() -> WarnedResult<(Vec<DatedNewsItem>, NewsCache), String, anyhow::Error> {
    todo!()
}

#[cfg(not(feature = "mock-api"))]
async fn get_news_online() -> WarnedResult<(Vec<DatedNewsItem>, NewsCache), String, anyhow::Error> {
    crate::news::get_news_online().await
}

#[cfg(feature = "mock-api")]
async fn get_updates_offline(_: &CacheState) -> arch_updates_rs::Result<Updates> {
    mock::get_mock_updates().await
}

#[cfg(not(feature = "mock-api"))]
async fn get_updates_offline(cache: &CacheState) -> arch_updates_rs::Result<Updates> {
    let CacheState {
        aur_cache,
        devel_cache,
        pacman_cache,
    } = cache;
    let (pacman, aur, devel) = join!(
        arch_updates_rs::check_pacman_updates_offline(pacman_cache),
        arch_updates_rs::check_aur_updates_offline(aur_cache),
        arch_updates_rs::check_devel_updates_offline(devel_cache),
    );
    Ok(Updates {
        pacman: pacman?,
        aur: aur?,
        devel: devel?,
    })
}

async fn get_updates_online() -> arch_updates_rs::Result<(Updates, CacheState)> {
    let (pacman, aur, devel) = join!(
        arch_updates_rs::check_pacman_updates_online(),
        arch_updates_rs::check_aur_updates_online(),
        arch_updates_rs::check_devel_updates_online(),
    );
    let (pacman, pacman_cache) = pacman?;
    let (aur, aur_cache) = aur?;
    let (devel, devel_cache) = devel?;
    Ok((
        Updates { pacman, aur, devel },
        CacheState {
            aur_cache,
            devel_cache,
            pacman_cache,
        },
    ))
}

#[cfg(feature = "mock-api")]
/// This module provides a way to feed mock data to the app when compiled with
/// the mock-api feature using the mock_updates.ron file.
mod mock {
    use super::Updates;
    use arch_updates_rs::{AurUpdate, DevelUpdate, PacmanUpdate, SourceRepo};
    use serde::Deserialize;

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
}
