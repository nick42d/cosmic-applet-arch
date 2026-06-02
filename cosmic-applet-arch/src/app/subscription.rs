use super::{CosmicAppletArch, Message, SUBSCRIPTION_BUF_SIZE};
use crate::core::config::Config;
use futures::Stream;
use std::sync::Arc;
use tokio::sync::Notify;

#[cfg(feature = "mock-api")]
/// This module provides a way to feed mock data to the app when compiled with
/// the mock-api feature using the mock_updates.ron and mock_news.ron files.
mod mock;

pub mod core;
mod messages_to_app;
mod news_worker;
mod updates_worker;

// Long running stream of messages to the app.
pub fn subscription(app: &CosmicAppletArch) -> cosmic::iced::Subscription<Message> {
    let refresh_pressed_notifier = app.refresh_pressed_notifier.clone();
    let clear_news_pressed_notifier = app.clear_news_pressed_notifier.clone();
    let config_arc = app.config.clone();
    let news_worker =
        |tx| news_worker::raw_news_worker(tx, clear_news_pressed_notifier, config_arc);
    let config_arc = app.config.clone();
    let updates_worker =
        |tx| updates_worker::raw_updates_worker(tx, refresh_pressed_notifier, config_arc);
    let updates_stream = cosmic::iced::stream::channel(SUBSCRIPTION_BUF_SIZE, updates_worker);
    let news_stream = cosmic::iced::stream::channel(SUBSCRIPTION_BUF_SIZE, news_worker);
    let updates_sub = cosmic::iced::Subscription::run(updates_stream);
    let news_sub = cosmic::iced::Subscription::run(news_stream);
    cosmic::iced::Subscription::batch([updates_sub, news_sub])
}

fn updates_stream_builder(data: &(Arc<Notify>, Arc<Config>)) -> impl Stream<Item = Message> {
    let (refresh_pressed_notifier, config) = data.to_owned();
    let updates_worker =
        |tx| updates_worker::raw_updates_worker(tx, refresh_pressed_notifier, config);
    cosmic::iced::stream::channel(SUBSCRIPTION_BUF_SIZE, updates_worker)
}

fn news_stream_builder(data: &(Arc<Notify>, Arc<Config>)) -> impl Stream<Item = Message> {
    let (clear_news_pressed_notifier, config) = data.to_owned();
    let news_worker = |tx| news_worker::raw_news_worker(tx, clear_news_pressed_notifier, config);
    cosmic::iced::stream::channel(SUBSCRIPTION_BUF_SIZE, news_worker)
}
