use crate::app::subscription::core::{
    ErrorVecWithHistory, OfflineUpdatesMessage, OnlineUpdatesMessage, UpdatesError,
};
use crate::core::config::{Config, UpdateType};
use crate::news::{self, DatedNewsItem};
use arch_updates_rs::{AurUpdate, DevelUpdate, PacmanUpdate};
use chrono::{DateTime, Local};
use cosmic::app::{Core, Task};
use cosmic::iced::platform_specific::shell::wayland::commands::popup::{destroy_popup, get_popup};
use cosmic::iced::window::Id;
use cosmic::iced::Limits;
use cosmic::{Application, Element};
use std::collections::HashSet;
use std::sync::Arc;
use view::Collapsed;

// See module docs.
#[cfg(all(unix, not(target_os = "solaris")))]
mod async_file_lock;
mod subscription;
mod view;

const SUBSCRIPTION_BUF_SIZE: usize = 10;

#[derive(Default)]
pub struct CosmicAppletArch {
    /// Required by libcosmic
    core: Core,
    /// Default field for cosmic applet
    popup: Option<Id>,
    pacman_list_state: Collapsed,
    aur_list_state: Collapsed,
    devel_list_state: Collapsed,
    refresh_pressed_notifier: Arc<tokio::sync::Notify>,
    clear_news_pressed_notifier: Arc<tokio::sync::Notify>,
    updates_refreshing: bool,
    news: NewsState,
    updates: UpdatesState,
    config: Arc<Config>,
}

#[derive(Default, Debug)]
pub enum UpdatesState {
    #[default]
    Init,
    Running {
        last_checked_online: chrono::DateTime<Local>,
        pacman: ErrorVecWithHistory<PacmanUpdate, UpdatesError>,
        aur: ErrorVecWithHistory<AurUpdate, UpdatesError>,
        devel: ErrorVecWithHistory<DevelUpdate, UpdatesError>,
    },
}

impl UpdatesState {
    /// Returns the total number of updates exluding the passed UpdateTypes.
    pub fn total_filtered(&self, exclude_from_count: &HashSet<UpdateType>) -> usize {
        let UpdatesState::Running {
            last_checked_online,
            pacman,
            aur,
            devel,
        } = self
        else {
            return 0;
        };
        let total_updates = |u: UpdateType| match u {
            UpdateType::Aur => aur.len(),
            UpdateType::Devel => devel.len(),
            UpdateType::Pacman => pacman.len(),
        };
        HashSet::from([UpdateType::Aur, UpdateType::Devel, UpdateType::Pacman])
            .difference(exclude_from_count)
            .fold(0, |acc, e| acc + total_updates(*e))
    }
}

#[derive(Default, Debug)]
pub enum NewsState {
    #[default]
    Init,
    InitError {
        error: String,
    },
    Received {
        last_checked_online: chrono::DateTime<Local>,
        value: Vec<news::DatedNewsItem>,
    },
    Clearing {
        last_checked_online: chrono::DateTime<Local>,
        last_value: Vec<DatedNewsItem>,
    },
    ClearingError {
        last_checked_online: chrono::DateTime<Local>,
        last_value: Vec<DatedNewsItem>,
    },
    Error {
        last_checked_online: chrono::DateTime<Local>,
        last_value: Vec<news::DatedNewsItem>,
        error: String,
    },
}

#[derive(Debug, Clone)]
pub enum Message {
    ForceGetUpdates,
    TogglePopup,
    ToggleCollapsible(CollapsibleType),
    PopupClosed(Id),
    RefreshedUpdatesOnline {
        updates: OnlineUpdatesMessage,
    },
    RefreshedUpdatesOffline {
        updates: OfflineUpdatesMessage,
    },
    CheckNewsMsg {
        news: Vec<news::DatedNewsItem>,
        checked_online_time: DateTime<Local>,
    },
    CheckNewsErrorsMsg(String),
    ClearNewsMsg,
    ClearNewsErrorMsg,
    CheckUpdatesErrorsMsg {
        error_string: String,
    },
    OpenUrl(String),
}

#[derive(Clone, Debug)]
pub enum CollapsibleType {
    Aur,
    Pacman,
    Devel,
}

impl Application for CosmicAppletArch {
    // Use the default Cosmic executor.
    type Executor = cosmic::executor::Default;
    // Config data type for init function.
    type Flags = Config;
    type Message = Message;
    const APP_ID: &'static str = "com.nick42d.CosmicAppletArch";

    // Required functions
    fn core(&self) -> &Core {
        &self.core
    }
    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }
    // Use default cosmic applet style
    fn style(&self) -> Option<cosmic::iced_runtime::Appearance> {
        Some(cosmic::applet::style())
    }
    // Entry point for libcosmic init.
    // Core is passed by libcosmic, and caller can pass some state in Flags.
    // On load we can immediately run an async task by returning a Task as the
    // second component of the tuple.
    fn init(core: Core, config: Self::Flags) -> (Self, Task<Self::Message>) {
        let app = CosmicAppletArch {
            core,
            config: Arc::new(config),
            ..Default::default()
        };
        (app, Task::none())
    }
    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }
    // view is what is displayed in the toolbar when run as an applet.
    fn view(&self) -> Element<Self::Message> {
        view::view(self)
    }
    // view_window is what is displayed in the popup.
    fn view_window(&self, id: Id) -> Element<Self::Message> {
        view::view_window::view_window(self, id)
    }
    // NOTE: Tasks may be returned for asynchronous execution on a
    // background thread managed by the application's executor.
    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::TogglePopup => self.handle_toggle_popup(),
            Message::PopupClosed(id) => self.handle_popup_closed(id),
            Message::RefreshedUpdatesOnline { updates } => {
                self.handle_updates(updates, checked_online_time)
            }
            Message::ForceGetUpdates => self.handle_force_get_updates(),
            Message::ToggleCollapsible(update_type) => self.handle_toggle_collapsible(update_type),
            Message::CheckUpdatesErrorsMsg { error_string } => {
                self.handle_update_error(error_string)
            }
            Message::OpenUrl(url) => self.handle_open_url(url),
            Message::CheckNewsMsg {
                news,
                checked_online_time,
            } => self.handle_check_news_msg(news, checked_online_time),
            Message::CheckNewsErrorsMsg(e) => self.handle_check_news_errors_msg(e),
            Message::ClearNewsMsg => self.handle_clear_news_msg(),
            Message::ClearNewsErrorMsg => self.handle_clear_news_error_msg(),
        }
    }
    // Long running stream of messages to the app.
    fn subscription(&self) -> cosmic::iced::Subscription<Self::Message> {
        subscription::subscription(self)
    }
}

impl CosmicAppletArch {
    fn handle_check_news_msg(
        &mut self,
        news: Vec<DatedNewsItem>,
        time: chrono::DateTime<Local>,
    ) -> Task<Message> {
        // TODO: Consider bouncing this task like we do in handle_updates.
        self.news = NewsState::Received {
            value: news,
            last_checked_online: time,
        };
        Task::none()
    }
    fn handle_check_news_errors_msg(&mut self, e: String) -> Task<Message> {
        let old_news = std::mem::take(&mut self.news);
        self.news = match old_news {
            NewsState::Init => NewsState::InitError { error: e },
            NewsState::InitError { .. } => NewsState::InitError { error: e },
            NewsState::Received {
                last_checked_online,
                value,
            } => NewsState::Error {
                last_checked_online,
                last_value: value,
                error: e,
            },
            NewsState::Clearing {
                last_value,
                last_checked_online,
            }
            | NewsState::ClearingError {
                last_value,
                last_checked_online,
            } => NewsState::Error {
                last_value,
                error: e,
                last_checked_online,
            },
            NewsState::Error {
                last_value,
                last_checked_online,
                ..
            } => NewsState::Error {
                last_value,
                error: e,
                last_checked_online,
            },
        };
        Task::none()
    }
    fn handle_clear_news_msg(&mut self) -> Task<Message> {
        let old_news = std::mem::take(&mut self.news);
        self.news = match old_news {
            NewsState::Init | NewsState::InitError { .. } => {
                eprintln!("Warning: Tried to clear news, but there wasn't any");
                old_news
            }
            NewsState::Received {
                last_checked_online,
                value,
            } => NewsState::Clearing {
                last_value: value,
                last_checked_online,
            },
            NewsState::Clearing {
                last_value,
                last_checked_online,
            } => NewsState::Clearing {
                last_value,
                last_checked_online,
            },
            NewsState::ClearingError {
                last_value,
                last_checked_online,
            } => NewsState::Clearing {
                last_value,
                last_checked_online,
            },
            NewsState::Error {
                last_value,
                error: _,
                last_checked_online,
            } => NewsState::Clearing {
                last_value,
                last_checked_online,
            },
        };
        self.clear_news_pressed_notifier.notify_one();
        Task::none()
    }
    fn handle_clear_news_error_msg(&mut self) -> Task<Message> {
        let old_news = std::mem::take(&mut self.news);
        self.news = match old_news {
            NewsState::Clearing {
                last_value,
                last_checked_online,
            }
            | NewsState::ClearingError {
                last_checked_online,
                last_value,
            } => NewsState::ClearingError {
                last_value,
                last_checked_online,
            },
            ref s => {
                eprintln!("WARNING: Recieved an error message that I was unable to clear news, but I wasn't clearing news. State: {:?}", s);
                old_news
            }
        };
        Task::none()
    }
    fn handle_open_url(&self, url: String) -> Task<Message> {
        if let Err(e) = open::that_detached(&url) {
            eprintln!("Error {e} opening url {url}")
        }
        Task::none()
    }
    fn handle_toggle_popup(&mut self) -> Task<Message> {
        if let Some(p) = self.popup.take() {
            destroy_popup(p)
        } else {
            self.pacman_list_state = Collapsed::Collapsed;
            self.aur_list_state = Collapsed::Collapsed;
            self.devel_list_state = Collapsed::Collapsed;
            let new_id = Id::unique();
            self.popup.replace(new_id);
            let mut popup_settings = self.core.applet.get_popup_settings(
                // Unwrap safety: this approach was used in the official cosmic applets
                // https://github.com/pop-os/cosmic-applets/commit/5b5cd77e7c75d0f5a8eab96231adca4cb7a02786#diff-644c3fce2a26d21e536fd2da1a183f63a2549053f1441dfe931286a115807916R309
                self.core.main_window_id().unwrap(),
                new_id,
                None,
                None,
                None,
            );
            popup_settings.positioner.size_limits = Limits::NONE
                .max_width(500.0)
                .min_width(300.0)
                .min_height(200.0)
                .max_height(1080.0);
            get_popup(popup_settings)
        }
    }
    fn handle_toggle_collapsible(&mut self, update_type: CollapsibleType) -> Task<Message> {
        match update_type {
            CollapsibleType::Aur => self.aur_list_state = self.aur_list_state.toggle(),
            CollapsibleType::Pacman => self.pacman_list_state = self.pacman_list_state.toggle(),
            CollapsibleType::Devel => self.devel_list_state = self.devel_list_state.toggle(),
        }
        Task::none()
    }
    fn handle_popup_closed(&mut self, id: Id) -> Task<Message> {
        if self.popup.as_ref() == Some(&id) {
            self.popup = None;
        }
        Task::none()
    }
    fn handle_force_get_updates(&mut self) -> Task<Message> {
        self.updates_refreshing = true;
        self.refresh_pressed_notifier.notify_one();
        Task::none()
    }
    fn handle_online_updates(&mut self, updates: OnlineUpdatesMessage) -> Task<Message> {
        self.updates_refreshing = false;
        // When first receiving updates, autosize will not trigger until the second
        // message is received. So, we intentionally bounce this message if it's
        // the first time updates have been received.
        let task = if matches!(self.updates, UpdatesState::Init) {
            Task::done(
                Message::RefreshedUpdatesOnline {
                    updates: updates.clone(),
                }
                .into(),
            )
        } else {
            Task::none()
        };
        let OnlineUpdatesMessage {
            pacman,
            aur,
            devel,
            update_time,
        } = updates;
        let prev_state = std::mem::take(&mut self.updates);
        self.updates = match prev_state {
            UpdatesState::Init => UpdatesState::Running {
                last_checked_online: update_time,
                pacman: ErrorVecWithHistory::new_from_result(pacman),
                aur: ErrorVecWithHistory::new_from_result(aur),
                devel: ErrorVecWithHistory::new_from_result(devel),
            },
            UpdatesState::Running {
                pacman: prev_pacman,
                aur: prev_aur,
                devel: prev_devel,
                ..
            } => UpdatesState::Running {
                last_checked_online: update_time,
                pacman: prev_pacman.replace_with_result_preserving_history(pacman),
                aur: prev_aur.replace_with_result_preserving_history(aur),
                devel: prev_devel.replace_with_result_preserving_history(devel),
            },
        };
        task
    }
    fn handle_offline_updates(&mut self, updates: OfflineUpdatesMessage) -> Task<Message> {
        let OfflineUpdatesMessage { pacman, aur, devel } = updates;
        let prev_state = std::mem::take(&mut self.updates);
        self.updates = match prev_state {
            UpdatesState::Init => {
                eprintln!(
                "WARNING: Offline update received by UI before it had received an online update"
            );
                prev_state
            }
            UpdatesState::Running {
                pacman: prev_pacman,
                aur: prev_aur,
                devel: prev_devel,
                last_checked_online,
            } => UpdatesState::Running {
                last_checked_online,
                pacman: prev_pacman.replace_with_option_result_preserving_history(pacman),
                aur: prev_aur.replace_with_option_result_preserving_history(aur),
                devel: prev_devel.replace_with_option_result_preserving_history(devel),
            },
        };
        Task::none()
    }
}
