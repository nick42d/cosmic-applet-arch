use chrono::{DateTime, Local};
use cosmic::app::{Command, Core};
use cosmic::iced::wayland::popup::{destroy_popup, get_popup};
use cosmic::iced::window::Id;
use cosmic::iced::Limits;
use cosmic::{Application, Element};
use std::sync::Arc;
use std::time::Duration;
use subscription::Updates;
use view::Collapsed;

mod subscription;
mod view;

/// How often to compare current packages with the latest version in memory.
const INTERVAL: Duration = Duration::from_secs(6);
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
    last_checked: Option<DateTime<Local>>,
    errors: Option<()>,
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
        errors: Option<()>,
    },
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
    fn style(
        &self,
    ) -> Option<<cosmic::Theme as cosmic::iced_style::application::StyleSheet>::Style> {
        // fn style(&self) -> Option<cosmic::iced_runtime::Appearance> {
        Some(cosmic::applet::style())
    }
    // Entry point for libcosmic init.
    // Core is passed by libcosmic, and caller can pass some state in Flags.
    // On load we can immediately run an async task by returning a Command as the
    // second component of the tuple.
    fn init(core: Core, _flags: Self::Flags) -> (Self, Command<Self::Message>) {
        let app = CosmicAppletArch {
            core,
            ..Default::default()
        };
        (app, Command::none())
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
    // NOTE: Commands may be returned for asynchronous execution on a
    // background thread managed by the application's executor.
    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::TogglePopup => self.handle_toggle_popup(),
            Message::PopupClosed(id) => self.handle_popup_closed(id),
            Message::CheckUpdatesMsg {
                updates,
                checked_online_time,
                errors,
            } => self.handle_updates(updates, checked_online_time, errors),
            Message::ForceGetUpdates => self.handle_force_get_updates(),
            Message::ToggleCollapsible(update_type) => self.handle_toggle_collapsible(update_type),
        }
    }
    // Long running stream of messages to the app.
    fn subscription(&self) -> cosmic::iced::Subscription<Self::Message> {
        subscription::subscription(self)
    }
}

impl CosmicAppletArch {
    fn handle_toggle_popup(&mut self) -> Command<Message> {
        if let Some(p) = self.popup.take() {
            destroy_popup(p)
        } else {
            self.pacman_list_state = Collapsed::Collapsed;
            self.aur_list_state = Collapsed::Collapsed;
            self.devel_list_state = Collapsed::Collapsed;
            let new_id = Id::unique();
            self.popup.replace(new_id);
            let mut popup_settings = self
                .core
                .applet
                // .get_popup_settings(Id::RESERVED, new_id, None, None, None);
                .get_popup_settings(Id::MAIN, new_id, None, None, None);
            popup_settings.positioner.size_limits = Limits::NONE
                .max_width(372.0)
                .min_width(300.0)
                .min_height(200.0)
                .max_height(1080.0);
            get_popup(popup_settings)
        }
    }
    fn handle_toggle_collapsible(&mut self, update_type: UpdateType) -> Command<Message> {
        match update_type {
            UpdateType::Aur => self.aur_list_state = self.aur_list_state.toggle(),
            UpdateType::Pacman => self.pacman_list_state = self.pacman_list_state.toggle(),
            UpdateType::Devel => self.devel_list_state = self.devel_list_state.toggle(),
        }
        Command::none()
    }
    fn handle_popup_closed(&mut self, id: Id) -> Command<Message> {
        if self.popup.as_ref() == Some(&id) {
            self.popup = None;
        }
        Command::none()
    }
    fn handle_force_get_updates(&mut self) -> Command<Message> {
        self.refresh_pressed_notifier.notify_one();
        Command::none()
    }
    fn handle_updates(
        &mut self,
        updates: Updates,
        time: Option<DateTime<Local>>,
        errors: Option<()>,
    ) -> Command<Message> {
        self.updates = Some(updates);
        if let Some(time) = time {
            self.last_checked = Some(time);
        }
        if let Some(errors) = errors {
            self.errors = Some(errors);
        }
        Command::none()
    }
}
