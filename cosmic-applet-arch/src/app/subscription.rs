use super::{CosmicAppletArch, Message, CYCLES, INTERVAL, SUBSCRIPTION_BUF_SIZE};
use arch_updates_rs::{CheckType, DevelUpdate, Update};
use chrono::Local;
use cosmic::iced::futures::{channel::mpsc, SinkExt};
use tokio::join;

// Long running stream of messages to the app.
pub fn subscription(app: &CosmicAppletArch) -> cosmic::iced::Subscription<Message> {
    let notifier = app.refresh_pressed_notifier.clone();
    let worker = |mut tx: mpsc::Sender<Message>| async move {
        let mut counter = 0;
        let mut cache = CacheState::default();
        let mut interval = tokio::time::interval(INTERVAL);
        loop {
            let notified = notifier.notified();
            tokio::select! {
                _ = interval.tick() => {
                    let check_type = match counter {
                        0 => arch_updates_rs::CheckType::Online,
                        _ => arch_updates_rs::CheckType::Offline(cache),
                    };
                    let (updates, cache_tmp) = get_updates_all(check_type).await;
                    cache = cache_tmp;
                    let checked_online_time =
                        if counter == 0 {
                            Some(Local::now())
                        } else {
                            None
                        };
                    tx.send(Message::CheckUpdatesMsg{
                        updates,
                        checked_online_time,
                        errors: None,
                    })
                    .await
                    .unwrap();
                    counter += 1;
                    if counter > CYCLES {
                        counter = 0
                    }
                }
                _ = notified => {
                    let (updates, cache_tmp) = get_updates_all(CheckType::Online).await;
                    cache = cache_tmp;
                    tx.send(Message::CheckUpdatesMsg{
                        updates,
                        checked_online_time: Some(Local::now()),
                        errors: None,
                    })
                    .await
                    .unwrap();
                    counter = 1;
                }
            }
        }
    };
    // subscription::Subscription::run(worker)
    cosmic::iced::subscription::channel(0, SUBSCRIPTION_BUF_SIZE, worker)
}

#[derive(Default)]
struct CacheState {
    aur_cache: Vec<Update>,
    devel_cache: Vec<DevelUpdate>,
}

#[derive(Clone, Debug, Default)]
pub struct Updates {
    pub pacman: Vec<Update>,
    pub aur: Vec<Update>,
    pub devel: Vec<DevelUpdate>,
}

async fn get_updates_all(check_type: CheckType<CacheState>) -> (Updates, CacheState) {
    match check_type {
        CheckType::Online => get_updates_online().await,
        CheckType::Offline(cache) => get_updates_offline(cache).await,
    }
}

async fn get_updates_offline(cache: CacheState) -> (Updates, CacheState) {
    let CacheState {
        aur_cache,
        devel_cache,
    } = cache;
    let (pacman, aur, devel) = join!(
        arch_updates_rs::check_updates(CheckType::Offline(())),
        arch_updates_rs::check_aur_updates(CheckType::Offline(aur_cache)),
        arch_updates_rs::check_devel_updates(CheckType::Offline(devel_cache)),
    );
    let (aur, aur_cache) = aur.unwrap();
    let (devel, devel_cache) = devel.unwrap();
    (
        Updates {
            pacman: pacman.unwrap(),
            aur,
            devel,
        },
        CacheState {
            aur_cache,
            devel_cache,
        },
    )
}

async fn get_updates_online() -> (Updates, CacheState) {
    let (pacman, aur, devel) = join!(
        arch_updates_rs::check_updates(CheckType::Online),
        arch_updates_rs::check_aur_updates(CheckType::Online),
        arch_updates_rs::check_devel_updates(CheckType::Online),
    );
    let (aur, aur_cache) = aur.unwrap();
    let (devel, devel_cache) = devel.unwrap();
    (
        Updates {
            pacman: pacman.unwrap(),
            aur,
            devel,
        },
        CacheState {
            aur_cache,
            devel_cache,
        },
    )
}
