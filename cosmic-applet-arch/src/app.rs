use chrono::{DateTime, Local};
use cosmic::app::{Core, Task};
use cosmic::iced::platform_specific::shell::wayland::commands::popup::{destroy_popup, get_popup};
use cosmic::iced::window::Id;
use cosmic::iced::Limits;
use cosmic::{Application, Element};
use std::sync::Arc;
use std::time::Duration;
use subscription::Updates;
use view::Collapsed;

use crate::news::{self, DatedNewsItem};

mod subscription;
mod view;

/// How often to compare current packages with the latest version in memory.
const INTERVAL: Duration = Duration::from_secs(6);
/// How long the api call can run without triggering a timeout.
const TIMEOUT: Duration = Duration::from_secs(60 * 2);
/// Every `CYCLES` number of `INTERVAL`s (starting at the first interval), the
/// system will update the latest version in memory from the internet.
const CYCLES: usize = 600;
const SUBSCRIPTION_BUF_SIZE: usize = 10;

#[derive(Default)]
pub struct CosmicAppletArch {
    /// Required by libcosmic
    core: Core,
    /// Default field for cosmic applet
    popup: Option<Id>,
    updates: Option<Updates>,
    pacman_list_state: Collapsed,
    aur_list_state: Collapsed,
    devel_list_state: Collapsed,
    refresh_pressed_notifier: Arc<tokio::sync::Notify>,
    clear_news_pressed_notifier: Arc<tokio::sync::Notify>,
    last_checked: Option<DateTime<Local>>,
    error: Option<String>,
    news: NewsState,
}

#[derive(Default, Debug)]
pub enum NewsState {
    #[default]
    Init,
    Received(Vec<news::DatedNewsItem>),
    Clearing {
        last_value: Vec<DatedNewsItem>,
    },
    ClearingError {
        last_value: Vec<DatedNewsItem>,
    },
    Error {
        last_value: Vec<news::DatedNewsItem>,
        error: String,
    },
}

#[derive(Debug, Clone)]
pub enum Message {
    ForceGetUpdates,
    TogglePopup,
    ToggleCollapsible(UpdateType),
    PopupClosed(Id),
    CheckUpdatesMsg {
        updates: Updates,
        checked_online_time: Option<DateTime<Local>>,
    },
    CheckNewsMsg(Vec<news::DatedNewsItem>),
    CheckNewsErrorsMsg(String),
    ClearNewsMsg,
    CheckUpdatesErrorsMsg {
        error_string: String,
        error_time: DateTime<Local>,
    },
    OpenUrl(String),
}

#[derive(Clone, Debug)]
pub enum UpdateType {
    Aur,
    Pacman,
    Devel,
}

impl Application for CosmicAppletArch {
    // Use the default Cosmic executor.
    type Executor = cosmic::executor::Default;
    // Config data type for init function.
    // TODO: Add configuration.
    type Flags = ();
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
    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let app = CosmicAppletArch {
            core,
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
        view::view_window(self, id)
    }
    // NOTE: Tasks may be returned for asynchronous execution on a
    // background thread managed by the application's executor.
    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::TogglePopup => self.handle_toggle_popup(),
            Message::PopupClosed(id) => self.handle_popup_closed(id),
            Message::CheckUpdatesMsg {
                updates,
                checked_online_time,
            } => self.handle_updates(updates, checked_online_time),
            Message::ForceGetUpdates => self.handle_force_get_updates(),
            Message::ToggleCollapsible(update_type) => self.handle_toggle_collapsible(update_type),
            Message::CheckUpdatesErrorsMsg {
                error_string,
                error_time,
            } => self.handle_update_error(error_string, error_time),
            Message::OpenUrl(url) => self.handle_open_url(url),
            Message::CheckNewsMsg(news) => self.handle_check_news_msg(news),
            Message::CheckNewsErrorsMsg(e) => self.handle_check_news_errors_msg(e),
            Message::ClearNewsMsg => self.handle_clear_news_msg(),
        }
    }
    // Long running stream of messages to the app.
    fn subscription(&self) -> cosmic::iced::Subscription<Self::Message> {
        subscription::subscription(self)
    }
}

impl CosmicAppletArch {
    fn handle_check_news_msg(&mut self, news: Vec<DatedNewsItem>) -> Task<Message> {
        // TODO: Consider bouncing this task like we do in handle_updates.
        self.news = NewsState::Received(news);
        Task::none()
    }
    fn handle_check_news_errors_msg(&mut self, e: String) -> Task<Message> {
        let old_news = std::mem::take(&mut self.news);
        self.news = match old_news {
            NewsState::Init => NewsState::Init,
            NewsState::Received(vec) => NewsState::Error {
                last_value: vec,
                error: e,
            },
            NewsState::Clearing { last_value } | NewsState::ClearingError { last_value } => {
                NewsState::Error {
                    last_value,
                    error: e,
                }
            }
            NewsState::Error { last_value, .. } => NewsState::Error {
                last_value,
                error: e,
            },
        };
        Task::none()
    }
    fn handle_clear_news_msg(&mut self) -> Task<Message> {
        let old_news = std::mem::take(&mut self.news);
        self.news = match old_news {
            NewsState::Init => NewsState::Init,
            NewsState::Received(vec) => NewsState::Clearing { last_value: vec },
            NewsState::Clearing { last_value } => NewsState::Clearing { last_value },
            NewsState::ClearingError { last_value } => NewsState::Clearing { last_value },
            NewsState::Error { last_value, error } => NewsState::Clearing { last_value },
        };
        self.clear_news_pressed_notifier.notify_one();
        Task::none()
    }
    fn handle_open_url(&self, url: String) -> Task<Message> {
        if let Err(e) = open::that(&url) {
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
                .max_width(444.0)
                .min_width(300.0)
                .min_height(200.0)
                .max_height(1080.0);
            get_popup(popup_settings)
        }
    }
    fn handle_toggle_collapsible(&mut self, update_type: UpdateType) -> Task<Message> {
        match update_type {
            UpdateType::Aur => self.aur_list_state = self.aur_list_state.toggle(),
            UpdateType::Pacman => self.pacman_list_state = self.pacman_list_state.toggle(),
            UpdateType::Devel => self.devel_list_state = self.devel_list_state.toggle(),
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
        self.refresh_pressed_notifier.notify_one();
        Task::none()
    }
    fn handle_update_error(&mut self, error: String, error_time: DateTime<Local>) -> Task<Message> {
        self.error = Some(error);
        self.last_checked = Some(error_time);
        Task::none()
    }
    fn handle_updates(&mut self, updates: Updates, time: Option<DateTime<Local>>) -> Task<Message> {
        // When first receiving updates, autosize will not trigger until the second
        // message is received. So, we intentionally bounce this message if it's
        // the first time updates have been received.
        let task: Task<Message> = if self.updates.is_none() {
            Task::done(cosmic::app::Message::App(Message::CheckUpdatesMsg {
                updates: updates.clone(),
                checked_online_time: time,
            }))
        } else {
            Task::none()
        };
        self.updates = Some(updates);
        if let Some(time) = time {
            self.last_checked = Some(time);
        }
        self.error = None;
        task
    }
}
