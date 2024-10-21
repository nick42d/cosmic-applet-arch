use std::sync::Arc;

use super::{CosmicAppletArch, Message, CYCLES, INTERVAL, SUBSCRIPTION_BUF_SIZE};
use arch_updates_rs::{CheckType, DevelUpdate, Update};
use chrono::Local;
use cosmic::iced::futures::{channel::mpsc, SinkExt};
use tokio::join;

// Long running stream of messages to the app.
pub fn subscription(app: &CosmicAppletArch) -> cosmic::iced::Subscription<Message> {
    let notifier = app.refresh_pressed_notifier.clone();
    let worker = |mut tx: mpsc::Sender<Message>| async move {
        let mut counter = 0;
        let mut cache = CacheState::default();
        let mut interval = tokio::time::interval(INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        // If we are in error state, that means the online check failed.
        // In that case we won't have a valid or up to date cache, so there is
        // no point checking offline either.
        // This will reset once we can run a succesful online check.
        let mut error_state = false;
        loop {
            let notified = notifier.notified();
            tokio::select! {
                _ = interval.tick() => {
                    let check_type = match counter {
                        0 => arch_updates_rs::CheckType::Online,
                        _ => arch_updates_rs::CheckType::Offline,
                    };
                    counter += 1;
                    if counter > CYCLES {
                        counter = 0
                    }
                    if error_state && matches!(check_type, CheckType::Offline) {
                        continue;
                    }
                    let updates = match check_type {
                        CheckType::Online => {
                            match get_updates_online().await {
                                Err(e) => {
                                    tx.send(Message::CheckUpdatesErrorsMsg(Arc::new(e)))
                                        .await
                                        .unwrap_or_else(|e| eprintln!("Error {e} sending Arch update status - maybe the applet has been dropped."));
                                    continue;
                                },
                                Ok((updates, cache_tmp)) => {
                                    error_state = false;
                                    cache = cache_tmp;
                                    updates
                                }
                            }
                        }
                        CheckType::Offline => {
                            match get_updates_offline(&cache).await {
                                Err(e) => {
                                    tx.send(Message::CheckUpdatesErrorsMsg(Arc::new(e)))
                                        .await
                                        .unwrap_or_else(|e| eprintln!("Error {e} sending Arch update status - maybe the applet has been dropped."));
                                    continue;
                                },
                                Ok(updates) => updates
                            }
                        }
                    };
                    let checked_online_time = match check_type {
                        CheckType::Online => Some(Local::now()),
                        CheckType::Offline => None,
                    };
                    tx.send(Message::CheckUpdatesMsg{
                            updates,
                            checked_online_time,
                        })
                        .await
                        .unwrap_or_else(|e| eprintln!("Error {e} sending Arch update status - maybe the applet has been dropped."));
                }
                _ = notified => {
                    let updates = get_updates_online().await;
                    match updates {
                        Ok((updates, cache_tmp)) => {
                            error_state = false;
                            cache = cache_tmp;
                            tx.send(Message::CheckUpdatesMsg{
                                updates,
                                checked_online_time: Some(Local::now()),
                            })
                            .await
                            .unwrap_or_else(|e| eprintln!("Error {e} sending Arch update status - maybe the applet has been dropped."))
                        },
                        Err(e) => {
                            tx.send(Message::CheckUpdatesErrorsMsg(Arc::new(e)))
                            .await
                            .unwrap_or_else(|e| eprintln!("Error {e} sending Arch update status - maybe the applet has been dropped."))
                        },
                    }
                    counter = 1;
                }
            }
        }
    };
    // subscription::Subscription::run(worker)
    cosmic::iced::subscription::channel(0, SUBSCRIPTION_BUF_SIZE, worker)
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

async fn get_updates_offline(cache: &CacheState) -> arch_updates_rs::Result<Updates> {
    let CacheState {
        aur_cache,
        devel_cache,
    } = cache;
    let (pacman, aur, devel) = join!(
        arch_updates_rs::check_updates(CheckType::Offline),
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
        arch_updates_rs::check_updates(CheckType::Online),
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
