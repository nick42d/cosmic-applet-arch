// SPDX-License-Identifier: GPL-3.0-only

use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::Duration;

use ::tokio::time::sleep;
use arch_updates_rs::{CheckType, DevelUpdate, Update};
use cosmic::app::{Command, Core};
use cosmic::applet::cosmic_panel_config::PanelSize;
use cosmic::applet::Size;
use cosmic::iced::alignment::{Horizontal, Vertical};
use cosmic::iced::futures::SinkExt;
use cosmic::iced::wayland::popup::{destroy_popup, get_popup};
use cosmic::iced::window::Id;
use cosmic::iced::{widget, Length, Limits};
use cosmic::iced_style::application;
use cosmic::theme::Button;
use cosmic::widget::settings;
use cosmic::{Application, Element, Theme};
use tokio::join;

use crate::fl;

/// This is the struct that represents your application.
/// It is used to define the data that will be used by your application.
#[derive(Default)]
pub struct CosmicAppletArch {
    /// Application state which is managed by the COSMIC runtime.
    core: Core,
    /// The popup id.
    popup: Option<Id>,
    /// Example row toggler.
    example_row: bool,
    //ADDED BY NICK42D
    icon: AppIcon,
    updates: Updates,
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
    CheckUpdatesMsg(Updates),
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
impl Application for CosmicAppletArch {
    // Use the default Cosmic executor.
    type Executor = cosmic::executor::Default;
    // Config data required prior to starting.
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

    // Use default cosmic style
    fn style(&self) -> Option<<Theme as application::StyleSheet>::Style> {
        Some(cosmic::applet::style())
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
        let pm = self.updates.pacman.len();
        let au = self.updates.aur.len();
        let dev = self.updates.devel.len();

        let total_updates = pm + au + dev;

        if total_updates > 0 {
            applet_button_with_text(self.core(), self.icon.to_str(), format!("{pm}/{au}/{dev}"))
                .on_press_down(Message::TogglePopup)
                .into()
        } else {
            self.core
                .applet
                .icon_button(self.icon.to_str())
                .on_press(Message::TogglePopup)
                .into()
        }
    }

    // view_window is what is displayed in the popup.
    fn view_window(&self, _id: Id) -> Element<Self::Message> {
        let content_list = cosmic::widget::list_column()
            .padding(5)
            .spacing(0)
            .add(settings::item(
                fl!("example-row"),
                widget::toggler(None, self.example_row, |value| {
                    Message::ToggleExampleRow(value)
                }),
            ));
        let pm = self.updates.pacman.len();
        let au = self.updates.aur.len();
        let dev = self.updates.devel.len();

        let total_updates = pm + au + dev;
        let content_list = if total_updates > 0 {
            content_list
                .add(cosmic::widget::text(format!("Pacman updates: {pm}")))
                .add(cosmic::widget::text(format!("Aur updates: {au}")))
                .add(cosmic::widget::text(format!("Dev updates: {dev}")))
        } else {
            content_list.add(cosmic::widget::text("No updates available"))
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
            Message::CheckUpdatesMsg(updates) => {
                let total = updates.pacman.len() + updates.aur.len() + updates.devel.len();
                if total == 0 {
                    self.icon = AppIcon::UpToDate
                } else {
                    self.icon = AppIcon::UpdatesAvailable
                }
                self.updates = updates;
            }
        }
        Command::none()
    }

    fn subscription(&self) -> cosmic::iced::Subscription<Self::Message> {
        const INTERVAL: Duration = Duration::from_secs(6);
        const CYCLES: usize = 600;
        const BUF_SIZE: usize = 10;
        cosmic::iced::subscription::channel(0, BUF_SIZE, |mut tx| async move {
            let mut counter = 0;
            let mut cache = CacheState::default();
            loop {
                let check_type = match counter {
                    0 => CheckType::Online,
                    _ => CheckType::Offline(cache),
                };
                let (output, cache_tmp) = get_updates_all(check_type).await;
                cache = cache_tmp;
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

#[derive(Default)]
struct CacheState {
    aur_cache: Vec<Update>,
    devel_cache: Vec<DevelUpdate>,
}

#[derive(Clone, Debug, Default)]
struct Updates {
    pacman: Vec<Update>,
    aur: Vec<Update>,
    devel: Vec<DevelUpdate>,
}

async fn get_updates_all(check_type: CheckType<CacheState>) -> (Updates, CacheState) {
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
    match check_type {
        CheckType::Online => get_updates_online().await,
        CheckType::Offline(cache) => get_updates_offline(cache).await,
    }
}

// Extension of applet context icon_button_from_handle function.
pub fn applet_button_with_text<'a, Message: 'static>(
    core: &Core,
    icon_name: impl AsRef<str>,
    text: impl ToString,
) -> cosmic::widget::Button<'a, Message> {
    // Hardcode to symbolic = true.
    let suggested = core.applet.suggested_size(true);
    let applet_padding = core.applet.suggested_padding(true);

    let (mut configured_width, mut configured_height) = core.applet.suggested_window_size();

    // Adjust the width to include padding and force the crosswise dim to match the
    // window size
    let is_horizontal = core.applet.is_horizontal();
    if is_horizontal {
        configured_width = NonZeroU32::new(suggested.0 as u32 + applet_padding as u32 * 2).unwrap();
    } else {
        configured_height =
            NonZeroU32::new(suggested.1 as u32 + applet_padding as u32 * 2).unwrap();
    }

    let icon = cosmic::widget::icon::from_name(icon_name.as_ref())
        .symbolic(true)
        .size(suggested.0)
        .into();
    let icon = cosmic::widget::icon(icon)
        .style(cosmic::theme::Svg::Custom(Rc::new(|theme| {
            cosmic::iced_style::svg::Appearance {
                color: Some(theme.cosmic().background.on.into()),
            }
        })))
        .width(Length::Fixed(suggested.0 as f32))
        .height(Length::Fixed(suggested.1 as f32))
        .into();
    let t = match core.applet.size {
        Size::PanelSize(PanelSize::XL) => cosmic::widget::text::title2,
        Size::PanelSize(PanelSize::L) => cosmic::widget::text::title3,
        Size::PanelSize(PanelSize::M) => cosmic::widget::text::title4,
        Size::PanelSize(PanelSize::S) => cosmic::widget::text::body,
        Size::PanelSize(PanelSize::XS) => cosmic::widget::text::body,
        Size::Hardcoded(_) => cosmic::widget::text,
    };
    let text = t(text.to_string()).font(cosmic::font::default()).into();
    cosmic::widget::button::custom(
        cosmic::widget::layer_container(
            cosmic::widget::row::with_children(vec![icon, text])
                .align_items(cosmic::iced::Alignment::Center)
                .spacing(2),
        )
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center)
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .width(Length::Fixed(configured_width.get() as f32 + 45.0))
    .height(Length::Fixed(configured_height.get() as f32))
    .style(Button::AppletIcon)
}
