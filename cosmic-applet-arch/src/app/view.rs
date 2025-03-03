use super::{CosmicAppletArch, Message, UpdatesState};
use crate::{app::NewsState, fl};
use chrono::{DateTime, Local};
use cosmic::{
    app::Core,
    iced::{
        alignment::{Horizontal, Vertical},
        Length,
    },
    theme::{self, Button},
    widget::Id,
    Application, Element,
};
use std::borrow::Cow;
use std::{rc::Rc, sync::LazyLock};

pub use widgets::*;
mod widgets;

const MAX_UPDATE_LINES: usize = 20;
const MAX_NEWS_LINES: usize = 3;

// This is the same mechanism the official cosmic applets use.
static AUTOSIZE_MAIN_ID: LazyLock<Id> = LazyLock::new(|| Id::new("autosize-main"));

enum AppIcon {
    Loading,
    Error,
    UpdatesAvailable,
    UpToDate,
}

impl AppIcon {
    fn to_str(&self) -> &'static str {
        match self {
            AppIcon::UpdatesAvailable => "software-update-available-symbolic",
            AppIcon::UpToDate => "emblem-default-symbolic",
            AppIcon::Loading => "emblem-synchronizing-symbolic",
            AppIcon::Error => "dialog-error-symbolic",
        }
    }
}

// view is what is displayed in the toolbar when run as an applet.
pub fn view(app: &CosmicAppletArch) -> Element<Message> {
    let icon = match &app.updates {
        UpdatesState::Init => AppIcon::Loading,
        UpdatesState::InitError { .. } | UpdatesState::Error { .. } => AppIcon::Error,
        UpdatesState::Received { value, .. }
        | UpdatesState::Refreshing {
            last_value: value, ..
        } => {
            if value.total() == 0 {
                AppIcon::UpToDate
            } else {
                AppIcon::UpdatesAvailable
            }
        }
    };
    // Seemed like I couldn't use a let-else here but I assume it will be possible
    // in future.
    let updates = if let UpdatesState::Received { value: updates, .. }
    | UpdatesState::Refreshing {
        last_value: updates,
        ..
    } = &app.updates
    {
        updates
    } else {
        return app
            .core
            .applet
            .icon_button(icon.to_str())
            .on_press_down(Message::TogglePopup)
            .into();
    };
    let total_updates = updates.total();

    // TODO: Set a width when layout is vertical, button should be same width as
    // others.
    cosmic::widget::autosize::autosize(
        if total_updates > 0 {
            applet_button_with_text(app.core(), icon.to_str(), format!("{total_updates}"))
                .on_press_down(Message::TogglePopup)
        } else {
            app.core
                .applet
                .icon_button(icon.to_str())
                .on_press_down(Message::TogglePopup)
        },
        AUTOSIZE_MAIN_ID.clone(),
    )
    .into()
}

// view_window is what is displayed in the popup.
pub fn view_window(app: &CosmicAppletArch, _id: cosmic::iced::window::Id) -> Element<Message> {
    fn last_checked_string(t: DateTime<Local>) -> String {
        fl!(
            "last-checked",
            dateTime = format!("{}", t.format("%x %-I:%M %p"))
        )
    }
    let cosmic::cosmic_theme::Spacing {
        space_xxs, space_s, ..
    } = theme::active().cosmic().spacing;
    let content_list = cosmic::widget::column()
        .spacing(space_xxs)
        .padding([space_xxs, 0]);

    let last_checked_row = match app.updates {
        UpdatesState::Init | UpdatesState::InitError { .. } => None,
        UpdatesState::Received {
            last_checked_online,
            ..
        }
        | UpdatesState::Refreshing {
            last_checked_online,
            ..
        }
        | UpdatesState::Error {
            last_checked_online,
            ..
        } => Some(
            cosmic::applet::menu_button(cosmic::widget::text::body(last_checked_string(
                last_checked_online,
            )))
            .on_press(Message::ForceGetUpdates),
        ),
    };
    let loading_row = if matches!(
        app.updates,
        UpdatesState::Init | UpdatesState::InitError { .. }
    ) {
        Some(body_text_row(fl!("loading")))
    } else {
        None
    };
    let errors_row = match &app.updates {
        UpdatesState::InitError { error } => Some(errors_row_widget(error)),
        // TODO: This should be a special case where the error is pressable.
        UpdatesState::Error { error, .. } => Some(errors_row_widget(error)),
        UpdatesState::Init | UpdatesState::Received { .. } | UpdatesState::Refreshing { .. } => {
            None
        }
    };

    let updates = match &app.updates {
        UpdatesState::Received { value, .. } => value,
        UpdatesState::Refreshing { last_value, .. } => last_value,
        UpdatesState::Error { last_value, .. } => last_value,
        UpdatesState::Init | UpdatesState::InitError { .. } => {
            let content_list = content_list
                .push_maybe(last_checked_row)
                .push_maybe(loading_row)
                .push_maybe(errors_row);
            return app.core.applet.popup_container(content_list).into();
        }
    };

    let pm = updates.pacman.len();
    let aur = updates.aur.len();
    let dev = updates.devel.len();

    let pacman_list = updates_available_widget(
        updates
            .pacman
            .iter()
            .map(DisplayPackage::from_pacman_update),
        &app.pacman_list_state,
        fl!(
            "updates-available",
            numberUpdates = pm,
            updateSource = "pacman"
        ),
        Message::ToggleCollapsible(crate::app::CollapsibleType::PacmanUpdates),
        MAX_UPDATE_LINES,
    );
    let aur_list = updates_available_widget(
        updates.aur.iter().map(DisplayPackage::from_aur_update),
        &app.aur_list_state,
        fl!(
            "updates-available",
            numberUpdates = aur,
            updateSource = "AUR"
        ),
        Message::ToggleCollapsible(crate::app::CollapsibleType::AurUpdates),
        MAX_UPDATE_LINES,
    );
    let devel_list = updates_available_widget(
        updates.devel.iter().map(DisplayPackage::from_devel_update),
        &app.devel_list_state,
        fl!(
            "updates-available",
            numberUpdates = dev,
            updateSource = "devel"
        ),
        Message::ToggleCollapsible(crate::app::CollapsibleType::DevelUpdates),
        MAX_UPDATE_LINES,
    );
    let news_row = match &app.news {
        NewsState::Init => None,
        NewsState::InitError { error } => Some(errors_row_widget(error)),
        NewsState::Received {
            last_checked_online,
            value,
        } => Some(
            cosmic::iced_widget::column![
                cosmic::applet::menu_button(cosmic::widget::text::body(fl!("news")))
                    .on_press(Message::ClearNewsMsg),
                news_list_widget(value.iter(), MAX_NEWS_LINES, space_xxs)
            ]
            .into(),
        ),
        NewsState::Clearing {
            last_value,
            last_checked_online,
        } => Some(
            cosmic::iced_widget::column![
                cosmic::iced_widget::row![
                    cosmic::widget::text::body("Icon here"),
                    cosmic::applet::menu_button(cosmic::widget::text::body(fl!("news")))
                        .on_press(Message::ClearNewsMsg),
                ],
                news_list_widget(last_value.iter(), MAX_NEWS_LINES, space_xxs)
            ]
            .into(),
        ),
        NewsState::ClearingError {
            last_value,
            last_checked_online,
        } => todo!(),
        NewsState::Error {
            last_value,
            error,
            last_checked_online,
        } => todo!(),
    };
    let news_divider =
        if last_checked_row.is_some() || loading_row.is_some() || errors_row.is_some() {
            Some(cosmic_applet_divider(space_s).into())
        } else {
            None
        };

    let total_updates = pm + aur + dev;
    let content_list = content_list
        .push_maybe((pm > 0).then_some(pacman_list))
        .push_maybe((aur > 0 && pm > 0).then_some(cosmic_applet_divider(space_s).into()))
        .push_maybe((aur > 0).then_some(aur_list))
        .push_maybe((dev > 0 && pm + aur > 0).then_some(cosmic_applet_divider(space_s).into()))
        .push_maybe((dev > 0).then_some(devel_list))
        .push_maybe((total_updates == 0).then_some(body_text_row(fl!("no-updates-available"))))
        .push(cosmic_applet_divider(space_s).into())
        .push_maybe(last_checked_row)
        .push_maybe(loading_row)
        .push_maybe(errors_row)
        .push_maybe(news_divider)
        .push_maybe(news_row);
    app.core.applet.popup_container(content_list).into()
}

// Extension of applet context icon_button_from_handle function.
pub fn applet_button_with_text<'a, Message: 'static>(
    core: &Core,
    icon_name: impl AsRef<str>,
    text: impl Into<Cow<'a, str>>,
) -> cosmic::widget::Button<'a, Message> {
    // Hardcode to symbolic = true.
    let suggested = core.applet.suggested_size(true);
    let (configured_width, _) = core.applet.suggested_window_size();

    let icon = cosmic::widget::icon::from_name(icon_name.as_ref())
        .symbolic(true)
        .size(suggested.0)
        .into();
    let icon = cosmic::widget::icon(icon)
        .class(cosmic::theme::Svg::Custom(Rc::new(|theme| {
            cosmic::widget::svg::Style {
                color: Some(theme.cosmic().background.on.into()),
            }
        })))
        .width(Length::Fixed(suggested.0 as f32))
        .height(Length::Fixed(suggested.1 as f32))
        .into();
    let text = core
        .applet
        .text(text)
        .wrapping(cosmic::iced_core::text::Wrapping::Glyph);
    // Column or row layout depends on panel position.
    // TODO: handle text overflow when vertical.
    let container = if core.applet.is_horizontal() {
        cosmic::widget::layer_container(
            cosmic::widget::row::with_children(vec![icon, text.into()])
                .align_y(cosmic::iced::Alignment::Center)
                .spacing(2),
        )
    } else {
        cosmic::widget::layer_container(
            cosmic::widget::column::with_children(vec![icon, text.into()])
                .align_x(cosmic::iced::Alignment::Center)
                .max_width(configured_width.get() as f32)
                .spacing(2),
        )
    }
    .align_x(Horizontal::Center.into())
    .align_y(Vertical::Center.into());
    cosmic::widget::button::custom(container).class(Button::AppletIcon)
}
