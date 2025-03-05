use super::messages_to_app::{send_update, send_update_error};
use super::{Message, CYCLES};
use crate::app::subscription::core::{
    flat_erased_timeout, get_updates_offline, get_updates_online, CheckType, OnlineUpdateResidual,
};
use crate::app::{INTERVAL, TIMEOUT};
use chrono::Local;
use cosmic::iced::futures::channel::mpsc;
use std::sync::Arc;
use tokio::sync::Notify;

pub async fn raw_updates_worker(
    mut tx: mpsc::Sender<Message>,
    refresh_pressed_notifier: Arc<Notify>,
) {
    let mut counter = 0;
    // If we have no cache, that means we haven't run a succesful online check.
    // Offline checks will be skipped until we can run one.
    let mut residual = None;
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
                match (&check_type, &residual) {
                    (CheckType::Online, _) => {
                        match flat_erased_timeout(TIMEOUT, get_updates_online()).await {
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
                        match flat_erased_timeout(TIMEOUT, get_updates_offline(&residual.cache)).await {
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
                let updates = flat_erased_timeout(TIMEOUT, get_updates_online()).await;
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
