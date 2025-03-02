use super::{CosmicAppletArch, Message, CYCLES, SUBSCRIPTION_BUF_SIZE};

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
    let news_worker = |mut tx| news_worker::raw_news_worker(tx, clear_news_pressed_notifier);
    let updates_worker = |mut tx| updates_worker::raw_updates_worker(tx, refresh_pressed_notifier);
    // TODO: Determine if INTERVAL is sufficient to prevent too many timeouts.
    let updates_stream =
        cosmic::iced_futures::stream::channel(SUBSCRIPTION_BUF_SIZE, updates_worker);
    let news_stream = cosmic::iced_futures::stream::channel(SUBSCRIPTION_BUF_SIZE, news_worker);
    let updates_sub = cosmic::iced::Subscription::run_with_id("arch-updates-sub", updates_stream);
    let news_sub = cosmic::iced::Subscription::run_with_id("arch-news-sub", news_stream);
    cosmic::iced::Subscription::batch([updates_sub, news_sub])
}
