use super::messages_to_app::{send_update, send_update_error};
use super::Message;
use crate::app::subscription::core::{
    check_pacman_updates_online_exclusive, flat_erased_timeout, get_updates_offline,
    get_updates_online, CheckType, ErrorVecWithHistory, OnlineUpdateResidual, Updates,
};
use crate::core::config::Config;
use arch_updates_rs::{AurUpdatesCache, DevelUpdatesCache, PacmanUpdatesCache};
use chrono::Local;
use cosmic::iced::futures::channel::mpsc;
use futures::join;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;

#[derive(Default)]
struct UpdatesWorkerCacheState {
    pacman_cache: Option<PacmanUpdatesCache>,
    aur_cache: Option<AurUpdatesCache>,
    devel_cache: Option<DevelUpdatesCache>,
}

async fn check_for_updates_online_and_send_to_app(
    timeout: std::time::Duration,
    mut tx: mpsc::Sender<Message>,
) -> UpdatesWorkerCacheState {
    let (pacman, aur, devel) = join!(
        // arch_updates_rs::check_pacman_updates_online doesn't handle multiple concurrent
        // processes.
        flat_erased_timeout(timeout, check_pacman_updates_online_exclusive()),
        flat_erased_timeout(timeout, arch_updates_rs::check_aur_updates_online()),
        flat_erased_timeout(timeout, arch_updates_rs::check_devel_updates_online()),
    );
    let update_time = Local::now();
    fn extract_cache_and_update<U, C>(
        update: Result<(Vec<U>, C), String>,
    ) -> (Option<C>, ErrorVecWithHistory<U>) {
        match update {
            Ok((update, cache)) => (Some(cache), ErrorVecWithHistory::Ok { value: update }),
            Err(error) => (None, ErrorVecWithHistory::Error { error }),
        }
    }
    let (pacman_cache, pacman_updates) = extract_cache_and_update(pacman);
    let (aur_cache, aur_updates) = extract_cache_and_update(aur);
    let (devel_cache, devel_updates) = extract_cache_and_update(devel);
    let updates = Updates {
        pacman: pacman_updates,
        aur: aur_updates,
        devel: devel_updates,
        last_updated: Some(update_time),
    };
    send_update(&mut tx, updates, update_time).await;
    UpdatesWorkerCacheState {
        pacman_cache,
        aur_cache,
        devel_cache,
    }
}

async fn check_for_updates_offline_and_send_to_app(
    cache: UpdatesWorkerCacheState,
    timeout: std::time::Duration,
    mut tx: mpsc::Sender<Message>,
) {
    let UpdatesWorkerCacheState{
        aur_cache,
        devel_cache,
        pacman_cache,
    } = cache;
    async fn flat_inject<T,U>(t: Option<T>, f: impl AsyncFn(&T) -> U) -> Option<U> {
        match t {
            Some(t) => Some(f(&t).await),
            None => None,
        }
    }
    let (pacman, aur, devel) = join!(
        flat_inject(pacman_cache, arch_updates_rs::check_pacman_updates_offline),
        flat_inject(aur_cache, arch_updates_rs::check_aur_updates_offline),
        flat_inject(devel_cache, arch_updates_rs::check_devel_updates_offline),
    );
    Ok(Updates {
        pacman: pacman?,
        aur: aur?,
        devel: devel?,
    })
    let update_time = Local::now();
    fn extract_cache_and_update<U, C>(
        update: Result<(Vec<U>, C), String>,
    ) -> (Option<C>, ErrorVecWithHistory<U>) {
        match update {
            Ok((update, cache)) => (Some(cache), ErrorVecWithHistory::Ok { value: update }),
            Err(error) => (None, ErrorVecWithHistory::Error { error }),
        }
    }
    let (pacman_cache, pacman_updates) = extract_cache_and_update(pacman);
    let (aur_cache, aur_updates) = extract_cache_and_update(aur);
    let (devel_cache, devel_updates) = extract_cache_and_update(devel);
    let updates = Updates {
        pacman: pacman_updates,
        aur: aur_updates,
        devel: devel_updates,
        last_updated: Some(update_time),
    };
    send_update(&mut tx, updates, update_time).await;
}

pub async fn raw_updates_worker(
    mut tx: mpsc::Sender<Message>,
    refresh_pressed_notifier: Arc<Notify>,
    config: Arc<Config>,
) {
    let timeout = Duration::from_secs(config.timeout_secs);
    let online_check_period = config.online_check_period;

    let mut counter = 0;
    let mut cache_state = UpdatesWorkerCacheState::default();

    let mut interval = tokio::time::interval(Duration::from_secs(config.interval_secs));
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
                if counter > online_check_period {
                    counter = 0
                }

                match &check_type {
                    CheckType::Online => {
                        cache_state = check_for_updates_online_and_send_to_app(timeout, tx).await;
                    }
                    CheckType::Offline => {
                        match flat_erased_timeout(timeout, get_updates_offline(&residual.cache)).await {
                            Err(e) => {
                                send_update_error(&mut tx, e).await;
                                continue;
                            },
                            Ok(updates) => send_update(&mut tx, updates, residual.time).await
                        };
                    }
                };
            }
            // App has forced an update
            _ = notified => {
                counter = 1;
                let updates = flat_erased_timeout(timeout, get_updates_online()).await;
                match updates {
                    Ok((updates, cache)) => {
                        let now = Local::now();
                        residual = Some(OnlineUpdateResidual { cache, time: now });
                        send_update(&mut tx, updates, now).await;
                    },
                    Err(e) => {
                        residual = None;
                        send_update_error(&mut tx, e).await;
                    }
                }
            }
        }
    }
}
