use crate::app::Message;
use crate::core::config::{AurHelper, Config};
use crate::fl;
use crate::CosmicAppletArch;
use cosmic::app::Core;
use cosmic::iced::window::{Id, Level};
use cosmic::iced::Length;
use cosmic::{theme, Application, Element};
use std::sync::Arc;

#[derive(Default)]
pub struct SettingsWindow {
    core: Core,
    terminal_input: String,
    aur_helper: AurHelper,
}

#[derive(Debug, Clone)]
pub enum SettingsMessage {
    SetTerminal(String),
    ToggleAurHelper,
    Save,
}

impl Application for SettingsWindow {
    type Executor = <CosmicAppletArch as Application>::Executor;
    type Flags = Arc<Config>;
    type Message = SettingsMessage;

    const APP_ID: &'static str = "com.nick42d.CosmicAppletArch.settings";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn style(&self) -> Option<cosmic::iced_runtime::Appearance> {
        Some(theme::active())
    }

    fn init(core: Core, config: Arc<Config>) -> (Self, cosmic::iced::Task<Self::Message>) {
        let mut settings_window = SettingsWindow {
            core,
            terminal_input: config.terminal.clone(),
            aur_helper: config.aur_helper,
        };
        if let Some(window_id) = settings_window.core.main_window_id() {
            settings_window
                .core
                .applet
                .window_settings(window_id)
                .level(Level::Fixed(Level::Normal))
                .title(fl!("settings-title"));
        }
        (settings_window, cosmic::iced::Task::none())
    }

    fn on_close_requested(&self, _id: Id) -> Option<Self::Message> {
        None
    }

    fn update(&mut self, message: Self::Message) -> cosmic::iced::Task<Self::Message> {
        match message {
            SettingsMessage::SetTerminal(term) => {
                self.terminal_input = term;
                cosmic::iced::Task::none()
            }
            SettingsMessage::ToggleAurHelper => {
                self.aur_helper = match self.aur_helper {
                    AurHelper::Yay => AurHelper::Paru,
                    AurHelper::Paru => AurHelper::Yay,
                };
                cosmic::iced::Task::none()
            }
            SettingsMessage::Save => cosmic::iced::Task::none(),
        }
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let cosmic::cosmic_theme::Spacing {
            space_s, space_m, ..
        } = theme::active().cosmic().spacing;

        let terminal_label = cosmic::widget::text::body(fl!("terminal-label"));
        let terminal_input = cosmic::widget::text_input::TextInput::new(
            "terminal",
            &self.terminal_input,
            "",
            SettingsMessage::SetTerminal,
        )
        .placeholder(fl!("terminal-placeholder"));

        let aur_helper_label = cosmic::widget::text::body(fl!("aur-helper-label"));
        let aur_helper_text = match self.aur_helper {
            AurHelper::Yay => "yay",
            AurHelper::Paru => "paru",
        };
        let aur_helper_button =
            cosmic::widget::button::custom(cosmic::widget::text::body(aur_helper_text))
                .on_press(SettingsMessage::ToggleAurHelper);

        let save_button = cosmic::widget::button::custom(cosmic::widget::text::body(fl!("save")))
            .on_press(SettingsMessage::Save);

        let content = cosmic::widget::column()
            .spacing(space_m)
            .padding(space_m)
            .push(terminal_label)
            .push(terminal_input)
            .push(aur_helper_label)
            .push(aur_helper_button)
            .push(save_button);

        cosmic::widget::window_container(
            cosmic::widget::container(content)
                .padding(space_s)
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .title(fl!("settings-title"))
        .into()
    }

    fn view_window(&self, id: Id) -> Element<'_, Self::Message> {
        self.view()
    }
}
