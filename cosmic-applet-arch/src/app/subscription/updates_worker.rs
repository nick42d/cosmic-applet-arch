use super::Message;
use crate::app::subscription::core::{
    get_updates_offline, get_updates_online, CacheState, CheckType,
};
use crate::app::subscription::messages_to_app::{send_offline_update, send_online_update};
use crate::core::config::Config;
use chrono::Local;
use cosmic::iced::futures::channel::mpsc;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;

pub async fn raw_updates_worker(
    mut tx: mpsc::Sender<Message>,
    refresh_pressed_notifier: Arc<Notify>,
    config: Arc<Config>,
) {
    let timeout = Duration::from_secs(config.timeout_secs);
    let online_check_period = config.online_check_period;

    let mut counter = 0;
    let mut cache_state = CacheState::default();

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
                        let updates;
                        (updates, cache_state) = get_updates_online(timeout).await;
                        let update_time = Local::now();
                        send_online_update(&mut tx, updates, update_time).await;
                    }
                    CheckType::Offline => {
                        let updates = get_updates_offline(&cache_state).await;
                        send_offline_update(&mut tx, updates).await;
                    }
                };
            }
            // App has forced an online update
            _ = refresh_pressed_notifier.notified() => {
                counter = 1;
                let updates;
                (updates, cache_state) = get_updates_online(timeout).await;
                let update_time = Local::now();
                send_online_update(&mut tx, updates, update_time).await;
            }
        }
    }
}
