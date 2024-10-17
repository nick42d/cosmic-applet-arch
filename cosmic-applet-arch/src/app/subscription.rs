use super::{CosmicAppletArch, Message, BUF_SIZE, CYCLES, INTERVAL};
use arch_updates_rs::{CheckType, DevelUpdate, Update};
use cosmic::iced::futures::SinkExt;
use std::time::{Duration, SystemTime};
use tokio::join;

// Long running stream of messages to the app.
pub fn subscription(app: &CosmicAppletArch) -> cosmic::iced::Subscription<Message> {
    cosmic::iced::subscription::channel(0, BUF_SIZE, |mut tx| async move {
        let mut counter = 0;
        let mut cache = CacheState::default();
        loop {
            let check_type = match counter {
                0 => arch_updates_rs::CheckType::Online,
                _ => arch_updates_rs::CheckType::Offline(cache),
            };
            let (output, cache_tmp) = get_updates_all(check_type).await;
            cache = cache_tmp;
            tx.send(Message::CheckUpdatesMsg(
                output,
                if counter == 0 {
                    Some(SystemTime::now())
                } else {
                    None
                },
                None,
            ))
            .await
            .unwrap();
            counter += 1;
            if counter > CYCLES {
                counter = 0
            }
            tokio::time::sleep(INTERVAL).await;
        }
    })
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
