use super::core::Updates;
use super::{CosmicAppletArch, Message, CYCLES, SUBSCRIPTION_BUF_SIZE};
use crate::app::{INTERVAL, TIMEOUT};
use crate::news::{set_news_last_read, NewsCache};
use crate::news::{DatedNewsItem, WarnedResult};
use arch_updates_rs::{
    AurUpdate, AurUpdatesCache, DevelUpdate, DevelUpdatesCache, PacmanUpdate, PacmanUpdatesCache,
};
use chrono::{DateTime, Local};
use cosmic::iced::futures::{channel::mpsc, SinkExt};
use futures::{FutureExt, TryFutureExt};
use std::future::Future;
use tokio::join;
pub async fn send_update_error(tx: &mut mpsc::Sender<Message>, e: impl std::fmt::Display) {
    tx.send(Message::CheckUpdatesErrorsMsg {
        error_string: format!("{e}"),
    })
    .await
    .unwrap_or_else(|e| {
        eprintln!("Error {e} sending Arch update status - maybe the applet has been dropped.")
    });
}
pub async fn send_update(
    tx: &mut mpsc::Sender<Message>,
    updates: Updates,
    checked_online_time: DateTime<Local>,
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
pub async fn send_news(
    tx: &mut mpsc::Sender<Message>,
    news: Vec<DatedNewsItem>,
    checked_online_time: DateTime<Local>,
) {
    tx.send(Message::CheckNewsMsg {
        news,
        checked_online_time,
    })
    .await
    .unwrap_or_else(|e| {
        eprintln!("Error {e} sending Arch news status - maybe the applet has been dropped.")
    });
}
pub async fn send_news_error(tx: &mut mpsc::Sender<Message>, e: impl std::fmt::Display) {
    tx.send(Message::CheckNewsErrorsMsg(format!("{e}")))
        .await
        .unwrap_or_else(|e| {
            eprintln!("Error {e} sending Arch news status - maybe the applet has been dropped.")
        });
}
