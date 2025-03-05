use crate::{
    app::{CosmicAppletArch, Message, NewsState, UpdatesState},
    fl,
};
use chrono::{DateTime, Local};
use cosmic::widget::Id;
use cosmic::{
    app::Core,
    iced::{
        alignment::{Horizontal, Vertical},
        Length,
    },
    theme::{self, Button},
    Application, Element,
};
use std::borrow::Cow;
use std::rc::Rc;
use std::sync::LazyLock;

pub use widgets::*;
/// What is display when opening the applet menu
pub mod view_window;
mod widgets;

const MAX_UPDATE_LINES: usize = 20;
const MAX_NEWS_LINES: usize = 3;

// This is the same mechanism the official cosmic applets use.
static AUTOSIZE_MAIN_ID: LazyLock<Id> = LazyLock::new(|| Id::new("autosize-main"));

pub enum AppIcon {
    Loading,
    Error,
    UpdatesAvailable,
    NewsAvailable,
    UpToDate,
}

impl AppIcon {
    fn to_str(&self) -> &'static str {
        match self {
            AppIcon::UpdatesAvailable => "software-update-available-symbolic",
            AppIcon::UpToDate => "emblem-default-symbolic",
            AppIcon::Loading => "emblem-synchronizing-symbolic",
            AppIcon::Error => "dialog-error-symbolic",
            AppIcon::NewsAvailable => "mail-message-new-symbolic",
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
    let additional_icon = match &app.news {
        NewsState::Init | NewsState::InitError { .. } => None,
        NewsState::Received { value: news, .. }
        | NewsState::Clearing {
            last_value: news, ..
        }
        | NewsState::ClearingError {
            last_value: news, ..
        }
        | NewsState::Error {
            last_value: news, ..
        } => {
            if !news.is_empty() {
                Some(AppIcon::NewsAvailable)
            } else {
                None
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
            applet_button_with_text(
                app.core(),
                icon,
                additional_icon,
                format!("{total_updates}"),
            )
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

pub fn applet_icon(core: &Core, icon_type: AppIcon) -> cosmic::widget::Icon {
    // Hardcode to symbolic = true.
    let suggested = core.applet.suggested_size(true);

    let icon = cosmic::widget::icon::from_name(icon_type.to_str())
        .symbolic(true)
        .size(suggested.0)
        .into();
    cosmic::widget::icon(icon)
        .class(cosmic::theme::Svg::Custom(Rc::new(|theme| {
            cosmic::widget::svg::Style {
                color: Some(theme.cosmic().background.on.into()),
            }
        })))
        .width(Length::Fixed(suggested.0 as f32))
        .height(Length::Fixed(suggested.1 as f32))
}

// Extension of applet context icon_button_from_handle function.
pub fn applet_button_with_text<'a, Message: 'static>(
    core: &Core,
    icon: AppIcon,
    additional_icon: Option<AppIcon>,
    text: impl Into<Cow<'a, str>>,
) -> cosmic::widget::Button<'a, Message> {
    let (configured_width, _) = core.applet.suggested_window_size();

    let icon = applet_icon(core, icon);
    let additional_icon = additional_icon.map(|additional_icon| applet_icon(core, additional_icon));
    let text = core
        .applet
        .text(text)
        .wrapping(cosmic::iced_core::text::Wrapping::Glyph);
    // Column or row layout depends on panel position.
    // TODO: handle text overflow when vertical.
    let container = if core.applet.is_horizontal() {
        cosmic::widget::layer_container(
            cosmic::widget::row::with_children(vec![icon.into(), text.into()])
                .push_maybe(additional_icon)
                .align_y(cosmic::iced::Alignment::Center)
                .spacing(2),
        )
    } else {
        cosmic::widget::layer_container(
            cosmic::widget::column::with_children(vec![icon.into(), text.into()])
                .push_maybe(additional_icon)
                .align_x(cosmic::iced::Alignment::Center)
                .max_width(configured_width.get() as f32)
                .spacing(2),
        )
    }
    .align_x(Horizontal::Center.into())
    .align_y(Vertical::Center.into());
    cosmic::widget::button::custom(container).class(Button::AppletIcon)
}
