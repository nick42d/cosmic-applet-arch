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
        loop {
            let notified = notifier.notified();
            tokio::select! {
                _ = interval.tick() => {
                    let check_type = match counter {
                        0 => arch_updates_rs::CheckType::Online,
                        // The reason we clone on every loop, is because the cache isn't returned if the check errors.
                        // TODO: This should be able to be resolved by modifying the library.
                        _ => arch_updates_rs::CheckType::Offline(cache.clone()),
                    };
                    let updates = get_updates_all(check_type).await;
                    let checked_online_time =
                        if counter == 0 {
                            Some(Local::now())
                        } else {
                            None
                        };
                    match updates {
                        Ok((updates, cache_tmp)) => {
                            cache = cache_tmp;
                            tx.send(Message::CheckUpdatesMsg{
                                updates,
                                checked_online_time,
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
                    counter += 1;
                    if counter > CYCLES {
                        counter = 0
                    }
                }
                _ = notified => {
                    let updates = get_updates_all(CheckType::Online).await;
                    match updates {
                        Ok((updates, cache_tmp)) => {
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

async fn get_updates_all(
    check_type: CheckType<CacheState>,
) -> arch_updates_rs::Result<(Updates, CacheState)> {
    match check_type {
        CheckType::Online => get_updates_online().await,
        CheckType::Offline(cache) => get_updates_offline(cache).await,
    }
}

async fn get_updates_offline(cache: CacheState) -> arch_updates_rs::Result<(Updates, CacheState)> {
    let CacheState {
        aur_cache,
        devel_cache,
    } = cache;
    let (pacman, aur, devel) = join!(
        arch_updates_rs::check_updates(CheckType::Offline(())),
        arch_updates_rs::check_aur_updates(CheckType::Offline(aur_cache)),
        arch_updates_rs::check_devel_updates(CheckType::Offline(devel_cache)),
    );
    let (aur, aur_cache) = aur?;
    let (devel, devel_cache) = devel?;
    Ok((
        Updates {
            pacman: pacman.unwrap(),
            aur,
            devel,
        },
        CacheState {
            aur_cache,
            devel_cache,
        },
    ))
}

async fn get_updates_online() -> arch_updates_rs::Result<(Updates, CacheState)> {
    let (pacman, aur, devel) = join!(
        arch_updates_rs::check_updates(CheckType::Online),
        arch_updates_rs::check_aur_updates(CheckType::Online),
        arch_updates_rs::check_devel_updates(CheckType::Online),
    );
    let (aur, aur_cache) = aur?;
    let (devel, devel_cache) = devel?;
    Ok((
        Updates {
            pacman: pacman.unwrap(),
            aur,
            devel,
        },
        CacheState {
            aur_cache,
            devel_cache,
        },
    ))
}
