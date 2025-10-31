use super::Message;
use crate::news::DatedNewsItem;
use chrono::{DateTime, Local};
use cosmic::iced::futures::channel::mpsc;
use cosmic::iced::futures::SinkExt;

pub async fn send_online_update(
    tx: &mut mpsc::Sender<Message>,
    updates: super::core::OnlineUpdatesMessage,
) {
    tx.send(Message::RefreshedUpdatesOnline { updates })
        .await
        .unwrap_or_else(|e| {
            eprintln!("Error {e} sending Arch update status - maybe the applet has been dropped.")
        });
}
pub async fn send_offline_update(
    tx: &mut mpsc::Sender<Message>,
    updates: super::core::OfflineUpdatesMessage,
) {
    tx.send(Message::RefreshedUpdatesOffline { updates })
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
pub async fn send_news_clearing_error(tx: &mut mpsc::Sender<Message>) {
    tx.send(Message::ClearNewsErrorMsg)
        .await
        .unwrap_or_else(|e| {
            eprintln!("Error {e} sending Arch news status - maybe the applet has been dropped.")
        });
}
