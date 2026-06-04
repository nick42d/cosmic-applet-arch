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

// An iced upgrade caused subscriptions to no longer be able to be passed
// dependencies unless they implement Hash. This is a similar workaround to the
// one used by some official cosmic applets and assumes the hash is used for
// uniquely identifying the subscription.
//
// https://github.com/pop-os/cosmic-applets/blob/8981b0b48ec21ac2c93cad56d82b33c6ef705888/cosmic-applet-notifications/src/subscriptions/notifications.rs#L43
//
// TODO: This is kind of a code smell, so consider instead generating the
// notifier inside the subscription and sending it to the app with the first
// message (see iced docs)
//
// https://docs.rs/iced/latest/iced/struct.Subscription.html#method.run
struct SubscriptionDepsWrapper {
    inner: (Arc<Notify>, Arc<Config>),
    identifier: &'static str,
}
impl SubscriptionDepsWrapper {
    // # Note
    // The identifier should be globally unique per subscription
    fn new(notifier: Arc<Notify>, config: Arc<Config>, identifier: &'static str) -> Self {
        Self {
            inner: (notifier, config),
            identifier,
        }
    }
}
impl std::hash::Hash for SubscriptionDepsWrapper {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.identifier.hash(state);
    }
}

// Long running stream of messages to the app.
pub fn subscription(app: &CosmicAppletArch) -> cosmic::iced::Subscription<Message> {
    let updates_sub = cosmic::iced::Subscription::run_with(
        SubscriptionDepsWrapper::new(
            app.refresh_pressed_notifier.clone(),
            app.config.clone(),
            "updates_subscription",
        ),
        updates_stream_builder,
    );
    let news_sub = cosmic::iced::Subscription::run_with(
        SubscriptionDepsWrapper::new(
            app.clear_news_pressed_notifier.clone(),
            app.config.clone(),
            "news_subscription",
        ),
        news_stream_builder,
    );
    cosmic::iced::Subscription::batch([updates_sub, news_sub])
}

fn updates_stream_builder(data: &SubscriptionDepsWrapper) -> impl Stream<Item = Message> {
    let (refresh_pressed_notifier, config) = data.inner.to_owned();
    let updates_worker =
        |tx| updates_worker::raw_updates_worker(tx, refresh_pressed_notifier, config);
    cosmic::iced::stream::channel(SUBSCRIPTION_BUF_SIZE, updates_worker)
}

fn news_stream_builder(data: &SubscriptionDepsWrapper) -> impl Stream<Item = Message> {
    let (clear_news_pressed_notifier, config) = data.inner.to_owned();
    let news_worker = |tx| news_worker::raw_news_worker(tx, clear_news_pressed_notifier, config);
    cosmic::iced::stream::channel(SUBSCRIPTION_BUF_SIZE, news_worker)
}
