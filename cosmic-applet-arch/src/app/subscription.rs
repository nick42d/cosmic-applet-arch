use crate::app::TIMEOUT;

use super::{CosmicAppletArch, Message, CYCLES, INTERVAL, SUBSCRIPTION_BUF_SIZE};
use arch_updates_rs::{DevelUpdate, Update};
use chrono::{DateTime, Local};
use cosmic::iced::futures::{channel::mpsc, SinkExt};
use futures::TryFutureExt;
use std::future::Future;
use tokio::join;

// Long running stream of messages to the app.
pub fn subscription(app: &CosmicAppletArch) -> cosmic::iced::Subscription<Message> {
    let notifier = app.refresh_pressed_notifier.clone();
    async fn send_error(tx: &mut mpsc::Sender<Message>, e: impl std::fmt::Display) {
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
    // TODO: Determine if INTERVAL is sufficient to prevent too many timeouts.
    let worker = |mut tx: mpsc::Sender<Message>| async move {
        let mut counter = 0;
        // If we have no cache, that means we haven't run a succesful online check.
        // Offline checks will be skipped until we can run one.
        let mut cache = None;
        let mut interval = tokio::time::interval(INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            let notified = notifier.notified();
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
                                    send_error(&mut tx, e).await;
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
                                    send_error(&mut tx, e).await;
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
                            send_error(&mut tx, e).await;
                        }
                    }
                }
            }
        }
    };
    // subscription::Subscription::run(worker)
    cosmic::iced::subscription::channel(0, SUBSCRIPTION_BUF_SIZE, worker)
}

#[derive(Clone, Copy, Debug)]
enum CheckType {
    Online,
    Offline,
}

#[derive(Default, Clone)]
struct CacheState {
    aur_cache: Vec<Update>,
    devel_cache: Vec<DevelUpdate>,
}

#[derive(Clone, Debug, Default)]
pub struct Updates {
    pub pacman: Vec<Update>,
    pub aur: Vec<Update>,
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

async fn get_updates_offline(cache: &CacheState) -> arch_updates_rs::Result<Updates> {
    let CacheState {
        aur_cache,
        devel_cache,
    } = cache;
    let (pacman, aur, devel) = join!(
        arch_updates_rs::check_pacman_updates_offline(),
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
    let (aur, aur_cache) = aur?;
    let (devel, devel_cache) = devel?;
    Ok((
        Updates {
            pacman: pacman?,
            aur,
            devel,
        },
        CacheState {
            aur_cache,
            devel_cache,
        },
    ))
}
