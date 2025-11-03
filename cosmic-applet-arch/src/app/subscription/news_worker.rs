use super::messages_to_app::{send_news, send_news_error};
use super::Message;
use crate::app::subscription::core::{
    consume_warning, flat_timeout, get_news_offline, get_news_online, CheckType, OnlineNewsResidual,
};
use crate::app::subscription::messages_to_app::send_news_clearing_error;
use crate::core::config::Config;
use crate::news::set_news_last_read;
use chrono::Local;
use cosmic::iced::futures::channel::mpsc;
use futures::FutureExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;

pub async fn raw_news_worker(
    mut tx: mpsc::Sender<Message>,
    clear_news_pressed_notifier: Arc<Notify>,
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
                match (&check_type, &residual) {
                    (CheckType::Online, _) => {
                        match flat_timeout(timeout, get_news_online().map(consume_warning)).await {
                            Err(e) => {
                                residual = None;
                                send_news_error(&mut tx, e).await;
                                continue;
                            },
                            Ok((updates, cache)) => {
                                let now = Local::now();
                                residual = Some( OnlineNewsResidual{ cache, time: now });
                                send_news(&mut tx, updates, now).await;
                            }
                        }
                    }
                    (CheckType::Offline, Some(residual)) => {
                        match flat_timeout(timeout, get_news_offline(&residual.cache).map(consume_warning)).await {
                            Err(e) => {
                                send_news_error(&mut tx, e).await;
                                continue;
                            },
                            Ok(updates) =>
                                send_news(&mut tx, updates, residual.time).await
                        }
                    }
                    (CheckType::Offline, None) => continue,
                };
            }
            // User has manually triggered clear.
            _ = clear_news_pressed_notifier.notified() => {
                counter = 1;
                // Don't allow user to clear news if we haven't checked online yet (shouldn't be literally possible...)
                if let Some(residual) = residual {
                    if let Err(e) = set_news_last_read(residual.time.into()).await {
                        eprintln!("WARN: Error storing local cache {e}");
                        // Note - this will only temporarily show to the user, until the online check below has been performed.
                        send_news_clearing_error(&mut tx).await;
                    }
                } else {
                    eprintln!("WARN: User cleared news before it had been checked online - shouldn't be possible!");
                }
                // The theory here, is that by running a get_news right after a set_news, this will clear the news (if there isn't any new on the server).
                // Otherwise, it will send the latest news (which, if we don't do here, is just going to trigger on the next online refresh anyway).
                match flat_timeout(timeout, get_news_online().map(consume_warning)).await {
                    Err(e) => {
                        residual = None;
                        send_news_error(&mut tx, e).await;
                        continue;
                    },
                    Ok((updates, cache)) => {
                        let now = Local::now();
                        residual = Some( OnlineNewsResidual{ cache, time: now });
                        send_news(&mut tx, updates, now).await;
                    }
                }
            }
        }
    }
}
