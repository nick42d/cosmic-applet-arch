use super::{AppIcon, CosmicAppletArch, Message, UpdatesState, AUTOSIZE_MAIN_ID};
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
