use super::messages_to_app::{send_news, send_news_error, send_update, send_update_error};
use super::{CosmicAppletArch, Message, CYCLES, SUBSCRIPTION_BUF_SIZE};
use crate::app::subscription::core::{
    consume_warning, flat_erased_timeout, get_news_offline, CheckType, OnlineNewsResidual,
};
use crate::app::{INTERVAL, TIMEOUT};
use crate::news::{get_news_online, set_news_last_read, NewsCache};
use crate::news::{DatedNewsItem, WarnedResult};
use arch_updates_rs::{
    AurUpdate, AurUpdatesCache, DevelUpdate, DevelUpdatesCache, PacmanUpdate, PacmanUpdatesCache,
};
use chrono::{DateTime, Local};
use cosmic::iced::futures::{channel::mpsc, SinkExt};
use futures::{FutureExt, TryFutureExt};
use std::future::Future;
use std::sync::Arc;
use tokio::join;
use tokio::sync::Notify;

pub async fn raw_news_worker(
    mut tx: mpsc::Sender<Message>,
    clear_news_pressed_notifier: Arc<Notify>,
) {
    let mut counter = 0;
    // If we have no cache, that means we haven't run a succesful online check.
    // Offline checks will be skipped until we can run one.
    let mut residual = None;
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
                match (&check_type, &residual) {
                    (CheckType::Online, _) => {
                        match flat_erased_timeout(TIMEOUT, get_news_online().map(consume_warning)).await {
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
                        match flat_erased_timeout(TIMEOUT, get_news_offline(&residual.cache).map(consume_warning)).await {
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
            _ = notified => {
                counter = 1; // ??
                // Don't allow user to clear news if we haven't checked online yet (shouldn't be literally possible...)
                if let Some(residual) = residual {
                    if let Err(e) = set_news_last_read(residual.time.into()).await {
                        todo!();
                    }
                }
                // The theory here, is that by running a get_news right after a set_news, this will clear the news (if there isn't any new on the server).
                // Otherwise, it will send the latest news (which, if we don't do here, is just going to trigger on the next online refresh anyway).
                match flat_erased_timeout(TIMEOUT, get_news_online().map(consume_warning)).await {
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
