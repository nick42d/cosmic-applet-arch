// SPDX-License-Identifier: GPL-3.0-only

use std::time::Duration;

use ::tokio::time::sleep;
use arch_updates_rs::{CheckType, Update};
use cosmic::app::{Command, Core};
use cosmic::iced::futures::SinkExt;
use cosmic::iced::wayland::popup::{destroy_popup, get_popup};
use cosmic::iced::window::Id;
use cosmic::iced::Limits;
use cosmic::iced_style::application;
use cosmic::widget::{self, settings};
use cosmic::{Application, Element, Theme};

use crate::fl;

/// This is the struct that represents your application.
/// It is used to define the data that will be used by your application.
#[derive(Default)]
pub struct CosmicAppArch {
    /// Application state which is managed by the COSMIC runtime.
    core: Core,
    /// The popup id.
    popup: Option<Id>,
    /// Example row toggler.
    example_row: bool,
    //ADDED BY NICK42D
    icon: AppIcon,
    updates_text: Option<Vec<String>>,
}

#[derive(Default)]
enum AppIcon {
    #[default]
    UpdatesAvailable,
    UpToDate,
}

impl AppIcon {
    fn to_str(&self) -> &'static str {
        match self {
            AppIcon::UpdatesAvailable => "software-update-available-symbolic",
            AppIcon::UpToDate => "emblem-default-symbolic",
        }
    }
    fn toggle(&self) -> Self {
        match self {
            AppIcon::UpdatesAvailable => AppIcon::UpToDate,
            AppIcon::UpToDate => AppIcon::UpdatesAvailable,
        }
    }
}

/// This is the enum that contains all the possible variants that your
/// application will need to transmit messages. This is used to communicate
/// between the different parts of your application. If your application does
/// not need to send messages, you can use an empty enum or `()`.
#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    ToggleExampleRow(bool),
    CheckUpdatesMsg(Vec<arch_updates_rs::Update>),
}

/// Implement the `Application` trait for your application.
/// This is where you define the behavior of your application.
///
/// The `Application` trait requires you to define the following types and
/// constants:
/// - `Executor` is the async executor that will be used to run your
///   application's commands.
/// - `Flags` is the data that your application needs to use before it starts.
/// - `Message` is the enum that contains all the possible variants that your
///   application will need to transmit messages.
/// - `APP_ID` is the unique identifier of your application.
impl Application for CosmicAppArch {
    type Executor = cosmic::executor::Default;

    type Flags = ();

    type Message = Message;

    const APP_ID: &'static str = "com.nick42d.CosmicAppletArch";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    /// This is the entry point of your application, it is where you initialize
    /// your application.
    ///
    /// Any work that needs to be done before the application starts should be
    /// done here.
    ///
    /// - `core` is used to passed on for you by libcosmic to use in the core of
    ///   your own application.
    /// - `flags` is used to pass in any data that your application needs to use
    ///   before it starts.
    /// - `Command` type is used to send messages to your application.
    ///   `Command::none()` can be used to send no messages to your application.
    fn init(core: Core, _flags: Self::Flags) -> (Self, Command<Self::Message>) {
        let app = CosmicAppArch {
            core,
            ..Default::default()
        };

        (app, Command::none())
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    /// This is the main view of your application, it is the root of your widget
    /// tree.
    ///
    /// The `Element` type is used to represent the visual elements of your
    /// application, it has a `Message` associated with it, which dictates
    /// what type of message it can send.
    ///
    /// To get a better sense of which widgets are available, check out the
    /// `widget` module.
    fn view(&self) -> Element<Self::Message> {
        match self.updates_text.as_ref() {
            Some(u) => {
                cosmic::widget::button::custom(self.core.applet.text(format!("{}", u.len())))
                    .on_press_down(Message::TogglePopup)
                    .style(cosmic::theme::Button::AppletIcon)
                    .into()
            }
            None => self
                .core
                .applet
                .icon_button(self.icon.to_str())
                .on_press(Message::TogglePopup)
                .into(),
        }
    }

    fn view_window(&self, _id: Id) -> Element<Self::Message> {
        let mut content_list = widget::list_column()
            .padding(5)
            .spacing(0)
            .add(settings::item(
                fl!("example-row"),
                widget::toggler(None, self.example_row, |value| {
                    Message::ToggleExampleRow(value)
                }),
            ));
        let content_list = match &self.updates_text {
            Some(updates) => {
                // Only show the first 5 - avoid massive list
                for update in updates.iter().take(5) {
                    content_list = content_list.add(cosmic::widget::text(update));
                }
                content_list
            }
            None => content_list.add(cosmic::widget::text("No updates")),
        };
        self.core.applet.popup_container(content_list).into()
    }

    /// Application messages are handled here. The application state can be
    /// modified based on what message was received. Commands may be
    /// returned for asynchronous execution on a background thread managed
    /// by the application's executor.
    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::TogglePopup => {
                self.icon = self.icon.toggle();
                return if let Some(p) = self.popup.take() {
                    destroy_popup(p)
                } else {
                    let new_id = Id::unique();
                    self.popup.replace(new_id);
                    let mut popup_settings =
                        self.core
                            .applet
                            .get_popup_settings(Id::MAIN, new_id, None, None, None);
                    popup_settings.positioner.size_limits = Limits::NONE
                        .max_width(372.0)
                        .min_width(300.0)
                        .min_height(200.0)
                        .max_height(1080.0);
                    get_popup(popup_settings)
                };
            }
            Message::PopupClosed(id) => {
                if self.popup.as_ref() == Some(&id) {
                    self.popup = None;
                }
            }
            Message::ToggleExampleRow(toggled) => self.example_row = toggled,
            Message::CheckUpdatesMsg(vec) => {
                self.updates_text = Some(vec.into_iter().map(update_to_string).collect())
            }
        }
        Command::none()
    }

    fn style(&self) -> Option<<Theme as application::StyleSheet>::Style> {
        Some(cosmic::applet::style())
    }

    fn subscription(&self) -> cosmic::iced::Subscription<Self::Message> {
        const INTERVAL: Duration = Duration::from_secs(6);
        const CYCLES: usize = 600;
        const BUF_SIZE: usize = 10;
        cosmic::iced::subscription::channel(0, BUF_SIZE, |mut tx| async move {
            let mut counter = 0;
            loop {
                let check_type = match counter {
                    0 => CheckType::Online,
                    _ => CheckType::Offline(()),
                };
                let output = arch_updates_rs::check_updates(check_type).await.unwrap();
                tx.send(Message::CheckUpdatesMsg(output)).await.unwrap();
                counter += 1;
                if counter > CYCLES {
                    counter = 0
                }
                sleep(INTERVAL).await;
            }
        })
    }
}

fn update_to_string(update: Update) -> String {
    format!(
        "{} {}-{}->{}-{}",
        update.pkgname, update.pkgver_cur, update.pkgrel_cur, update.pkgver_new, update.pkgrel_new
    )
}
