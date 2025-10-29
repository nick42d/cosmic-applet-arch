use super::messages_to_app::{send_update, send_update_error};
use super::Message;
use crate::app::subscription::core::{
    check_pacman_updates_online_exclusive, flat_erased_timeout, get_updates_offline, get_updates_online, CheckType, ErrorVecWithHistory, OnlineUpdateResidual
};
use crate::core::config::Config;
use arch_updates_rs::{AurUpdatesCache, DevelUpdatesCache, PacmanUpdatesCache};
use chrono::Local;
use cosmic::iced::futures::channel::mpsc;
use futures::join;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;

enum UpdatesWorkerState {
    Init,
    CheckedOnline {
        pacman_cache: Option<PacmanUpdatesCache>,
        aur_cache: Option<AurUpdatesCache>,
        devel_cache: Option<DevelUpdatesCache>,
        last_online_check: chrono::DateTime<Local>,
    },
}

async fn check_for_updates_online_and_send_to_app(
    timeout: std::time::Duration,
    mut tx: mpsc::Sender<Message>,
) -> UpdatesWorkerState {
    let (pacman, aur, devel) = join!(
        // arch_updates_rs::check_pacman_updates_online doesn't handle multiple concurrent
        // processes.
        flat_erased_timeout(timeout, check_pacman_updates_online_exclusive()),
        flat_erased_timeout(timeout, arch_updates_rs::check_aur_updates_online()),
        flat_erased_timeout(timeout, arch_updates_rs::check_devel_updates_online()),
    );
    fn extract_update_and_cache(u: Result<(update, cache), E> ->
        ErrorVecWithHistory
    }
    match pacman {
        Ok(_) => todo!(),
        Err(_) => todo!(),
    };
    match aur {
        Ok(_) => todo!(),
        Err(_) => todo!(),
    };
    match devel {
        Ok(_) => todo!(),
        Err(_) => todo!(),
    };
    send_update_error(&mut tx, e).await;
    send_update(&mut tx, updates, now).await;
    UpdatesWorkerState::CheckedOnline {
        pacman_cache: (),
        aur_cache: (),
        devel_cache: (),
        last_online_check: Local::now(),
    }
}

pub async fn raw_updates_worker(
    mut tx: mpsc::Sender<Message>,
    refresh_pressed_notifier: Arc<Notify>,
    config: Arc<Config>,
) {
    let mut counter = 0;
    // If we have no cache, that means we haven't run a succesful online check.
    // Offline checks will be skipped until we can run one.
    let mut residual = None;
    let mut interval = tokio::time::interval(Duration::from_secs(config.interval_secs));
    let timeout = Duration::from_secs(config.timeout_secs);
    let online_check_period = config.online_check_period;
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
                if counter > online_check_period{
                    counter = 0
                }
                match (&check_type, &residual) {
                    (CheckType::Online, _) => {
                        match flat_erased_timeout(timeout, get_updates_online()).await {
                            Err(e) => {
                                residual = None;
                                send_update_error(&mut tx, e).await;
                                continue;
                            },
                            Ok((updates, cache)) => {
                                let now = Local::now();
                                residual = Some(OnlineUpdateResidual { cache, time: now});
                                send_update(&mut tx, updates, now).await;
                            }
                        }
                    }
                    (CheckType::Offline, Some(residual)) => {
                        match flat_erased_timeout(timeout, get_updates_offline(&residual.cache)).await {
                            Err(e) => {
                                send_update_error(&mut tx, e).await;
                                continue;
                            },
                            Ok(updates) => send_update(&mut tx, updates, residual.time).await
                        };
                    }
                    (CheckType::Offline, None) => continue,
                };
            }
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
