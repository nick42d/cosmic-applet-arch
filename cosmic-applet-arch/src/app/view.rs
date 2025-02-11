use super::{CosmicAppletArch, Message};
use crate::fl;
use arch_updates_rs::{DevelUpdate, Update};
use cosmic::{
    app::Core,
    iced::{
        alignment::{Horizontal, Vertical},
        Length,
    },
    theme::{self, Button},
    widget::{Id, JustifyContent, Widget},
    Application, Element,
};
use std::{borrow::Cow, fmt::Display};
use std::{rc::Rc, sync::LazyLock};

use widgets::*;
mod widgets;

const MAX_LINES: usize = 20;

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
    let mut icon = if app.error.is_some() {
        AppIcon::Error
    } else {
        AppIcon::Loading
    };

    let Some(updates) = app.updates.as_ref() else {
        return app
            .core
            .applet
            .icon_button(icon.to_str())
            .on_press_down(Message::TogglePopup)
            .into();
    };

    let total_updates = updates.pacman.len() + updates.aur.len() + updates.devel.len();

    if app.error.is_none() {
        if total_updates > 0 {
            icon = AppIcon::UpdatesAvailable;
        } else {
            icon = AppIcon::UpToDate;
        }
    }

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
    let cosmic::cosmic_theme::Spacing {
        space_xxs, space_s, ..
    } = theme::active().cosmic().spacing;
    let content_list = cosmic::widget::column()
        .spacing(space_xxs)
        .padding([space_xxs, 0]);

    let Some(updates) = app.updates.as_ref() else {
        let content_list = content_list.push(body_text_row(fl!("loading")));
        return app.core.applet.popup_container(content_list).into();
    };

    let pm = updates.pacman.len();
    let aur = updates.aur.len();
    let dev = updates.devel.len();

    let pacman_list = collapsible_two_column_package_list_widget(
        updates.pacman.iter().map(pretty_print_update),
        &app.pacman_list_state,
        fl!(
            "updates-available",
            numberUpdates = pm,
            updateSource = "pacman"
        ),
        Message::ToggleCollapsible(crate::app::UpdateType::Pacman),
        MAX_LINES,
    );
    let aur_list = collapsible_two_column_package_list_widget(
        updates.aur.iter().map(pretty_print_update),
        &app.aur_list_state,
        fl!(
            "updates-available",
            numberUpdates = aur,
            updateSource = "AUR"
        ),
        Message::ToggleCollapsible(crate::app::UpdateType::Aur),
        MAX_LINES,
    );
    let devel_list = collapsible_two_column_package_list_widget(
        updates.devel.iter().map(pretty_print_devel_update),
        &app.devel_list_state,
        fl!(
            "updates-available",
            numberUpdates = dev,
            updateSource = "devel"
        ),
        Message::ToggleCollapsible(crate::app::UpdateType::Devel),
        MAX_LINES,
    );

    let last_checked = match app.last_checked {
        Some(t) => format!("{}", t.format("%x %-I:%M %p")),
        None => fl!("not-yet"),
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
        .push(
            cosmic::applet::menu_button(cosmic::widget::text::body(fl!(
                "last-checked",
                dateTime = last_checked
            )))
            .on_press(Message::ForceGetUpdates),
        )
        .push_maybe(app.error.as_ref().map(errors_row));
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
