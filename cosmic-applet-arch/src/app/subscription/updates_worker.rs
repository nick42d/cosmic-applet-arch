use super::messages_to_app::send_update_error;
use super::Message;
use crate::app::subscription::core::{
    check_pacman_updates_online_exclusive, flat_timeout, get_updates_offline, CheckType,
    ErrorVecWithHistory, OfflineUpdatesMessage, OnlineUpdateResidual, OnlineUpdatesMessage,
    UpdatesError,
};
use crate::app::subscription::messages_to_app::{send_offline_update, send_online_update};
use crate::core::config::Config;
use arch_updates_rs::{AurUpdatesCache, DevelUpdatesCache, PacmanUpdatesCache};
use chrono::Local;
use cosmic::iced::futures::channel::mpsc;
use futures::join;
use std::error::Error;
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
    tx: &mut mpsc::Sender<Message>,
) -> UpdatesWorkerCacheState {
    let (pacman, aur, devel) = join!(
        // arch_updates_rs::check_pacman_updates_online doesn't handle multiple concurrent
        // processes.
        flat_timeout(timeout, check_pacman_updates_online_exclusive()),
        flat_timeout(timeout, arch_updates_rs::check_aur_updates_online()),
        flat_timeout(timeout, arch_updates_rs::check_devel_updates_online()),
    );
    let update_time = Local::now();
    fn extract_cache_and_update<U, C, E>(update: Result<(U, C), E>) -> (Option<C>, Result<U, E>) {
        match update {
            Ok((update, cache)) => (Some(cache), Ok(update)),
            Err(e) => (None, Err(e)),
        }
    }
    let (pacman_cache, pacman_updates) = extract_cache_and_update(pacman);
    let (aur_cache, aur_updates) = extract_cache_and_update(aur);
    let (devel_cache, devel_updates) = extract_cache_and_update(devel);
    let updates = OnlineUpdatesMessage {
        pacman: pacman_updates.map_err(|_| UpdatesError),
        aur: aur_updates.map_err(|_| UpdatesError),
        devel: devel_updates.map_err(|_| UpdatesError),
        update_time,
    };
    send_online_update(&mut tx, updates, update_time).await;
    UpdatesWorkerCacheState {
        pacman_cache,
        aur_cache,
        devel_cache,
    }
}

async fn check_for_updates_offline_and_send_to_app(
    cache: &UpdatesWorkerCacheState,
    timeout: std::time::Duration,
    tx: &mut mpsc::Sender<Message>,
) {
    let UpdatesWorkerCacheState {
        aur_cache,
        devel_cache,
        pacman_cache,
    } = cache;
    async fn flat_inject<T, U>(t: &Option<T>, f: impl AsyncFn(&T) -> U) -> Option<U> {
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
    let pacman = pacman.map(|r| r.map_err(|_| UpdatesError));
    let aur = aur.map(|r| r.map_err(|_| UpdatesError));
    let devel = devel.map(|r| r.map_err(|_| UpdatesError));
    let updates = OfflineUpdatesMessage { pacman, aur, devel };
    send_offline_update(&mut tx, updates).await;
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
                        cache_state = check_for_updates_online_and_send_to_app(timeout, &mut tx).await;
                    }
                    CheckType::Offline => {
                        check_for_updates_offline_and_send_to_app(&cache_state, timeout, &mut tx).await;
                    }
                };
            }
            // App has forced an online update
            _ = refresh_pressed_notifier.notified() => {
                counter = 1;
                cache_state = check_for_updates_online_and_send_to_app(timeout, &mut tx).await;
            }
        }
    }
}
