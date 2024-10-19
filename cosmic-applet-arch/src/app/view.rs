use crate::fl;

use super::{CosmicAppletArch, Message};
use arch_updates_rs::{DevelUpdate, Update};
use cosmic::{
    app::Core,
    applet::{cosmic_panel_config::PanelSize, Size},
    cosmic_theme::palette::convert::TryIntoColor,
    iced::{
        alignment::{Horizontal, Vertical},
        widget,
        window::Id,
        Length, Padding,
    },
    iced_widget::{column, row},
    theme::{self, Button},
    widget::{flex_row, settings, JustifyContent, Widget},
    Also, Application, Apply, Element,
};
use itertools::Itertools;
use std::num::NonZeroU32;
use std::rc::Rc;
use time::OffsetDateTime;

enum AppIcon {
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
}

// view is what is displayed in the toolbar when run as an applet.
pub fn view(app: &CosmicAppletArch) -> Element<Message> {
    let total_updates = app.updates.pacman.len() + app.updates.aur.len() + app.updates.devel.len();

    if total_updates > 0 {
        applet_button_with_text(
            app.core(),
            AppIcon::UpdatesAvailable.to_str(),
            format!("{total_updates}"),
        )
        .on_press(Message::TogglePopup)
        .into()
    } else {
        app.core
            .applet
            .icon_button(AppIcon::UpToDate.to_str())
            .on_press(Message::TogglePopup)
            .into()
    }
}

// view_window is what is displayed in the popup.
pub fn view_window(app: &CosmicAppletArch, _id: Id) -> Element<Message> {
    let cosmic::cosmic_theme::Spacing {
        space_xxs, space_s, ..
    } = theme::active().cosmic().spacing;

    const MAX_LINES: usize = 5;

    let pm = app.updates.pacman.len();
    let aur = app.updates.aur.len();
    let dev = app.updates.devel.len();

    let pacman_list = collapsible_two_column_list(
        app.updates.pacman.iter().map(pretty_print_update),
        &app.pacman_list_state,
        fl!(
            "updates-available",
            numberUpdates = pm,
            updateSource = "pacman"
        ),
        Message::ToggleCollapsible(crate::app::UpdateType::Pacman),
    );
    let aur_list = collapsible_two_column_list(
        app.updates.aur.iter().map(pretty_print_update),
        &app.aur_list_state,
        fl!(
            "updates-available",
            numberUpdates = aur,
            updateSource = "AUR"
        ),
        Message::ToggleCollapsible(crate::app::UpdateType::Aur),
    );
    let devel_list = collapsible_two_column_list(
        app.updates.devel.iter().map(pretty_print_devel_update),
        &app.devel_list_state,
        fl!(
            "updates-available",
            numberUpdates = dev,
            updateSource = "devel"
        ),
        Message::ToggleCollapsible(crate::app::UpdateType::Devel),
    );

    let last_checked = match app.last_checked {
        Some(t) => OffsetDateTime::format(
            t,
            &time::format_description::parse("[day]/[month] [hour]:[minute]").unwrap(),
        )
        .unwrap(),
        None => "Not yet".to_owned(),
    };

    let total_updates = pm + aur + dev;
    let content_list = cosmic::widget::column()
        .padding(5)
        .spacing(space_xxs)
        .push_maybe((pm > 0).then_some(pacman_list))
        .push_maybe(
            (aur > 0 && pm > 0).then_some(
                cosmic::applet::padded_control(cosmic::widget::divider::horizontal::default())
                    .padding([space_xxs, space_s]),
            ),
        )
        .push_maybe((aur > 0).then_some(aur_list))
        .push_maybe(
            (dev > 0 && pm + aur > 0).then_some(
                cosmic::applet::padded_control(cosmic::widget::divider::horizontal::default())
                    .padding([space_xxs, space_s]),
            ),
        )
        .push_maybe((dev > 0).then_some(devel_list))
        .push_maybe(
            (total_updates == 0).then_some(body_text_row("No updates available".to_string())),
        )
        .push(
            cosmic::applet::padded_control(cosmic::widget::divider::horizontal::default())
                .padding([space_xxs, space_s]),
        )
        .push(
            cosmic::applet::menu_button(cosmic::widget::text::body(fl!(
                "last-checked",
                dateTime = last_checked
            )))
            .on_press(Message::ForceGetUpdates),
        )
        .push_maybe(app.errors.map(|e| errors_row("Testing".to_string())));
    app.core.applet.popup_container(content_list).into()
}

#[derive(Default)]
pub enum Collapsed {
    #[default]
    Collapsed,
    Expanded,
}

impl Collapsed {
    pub fn toggle(&self) -> Self {
        match self {
            Collapsed::Collapsed => Collapsed::Expanded,
            Collapsed::Expanded => Collapsed::Collapsed,
        }
    }
}

fn body_text_row(text: String) -> Element<'static, Message> {
    cosmic::widget::container(
        cosmic::widget::text::body(text)
            .width(Length::Fill)
            .height(Length::Fixed(24.0))
            .align_y(Vertical::Center),
    )
    .padding(cosmic::applet::menu_control_padding())
    .into()
}

fn errors_row(error: String) -> Element<'static, Message> {
    cosmic::widget::container(
        cosmic::widget::text::body(format!("Warning: {error}!!"))
            .width(Length::Fill)
            .height(Length::Fixed(24.0))
            .align_y(Vertical::Center),
    )
    .padding(cosmic::applet::menu_control_padding())
    .into()
}

fn collapsible_two_column_list<'a>(
    text: impl Iterator<Item = (String, String)> + 'a,
    collapsed: &Collapsed,
    title: String,
    on_press_mesage: Message,
) -> Element<'a, Message> {
    let icon_name = match collapsed {
        Collapsed::Collapsed => "go-down-symbolic",
        Collapsed::Expanded => "go-up-symbolic",
    };
    let heading = cosmic::applet::menu_button(row![
        cosmic::widget::text::body(title)
            .width(Length::Fill)
            .height(Length::Fixed(24.0))
            .align_y(Vertical::Center),
        cosmic::widget::container(
            cosmic::widget::icon::from_name(icon_name)
                .size(16)
                .symbolic(true)
        )
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center)
        .width(Length::Fixed(24.0))
        .height(Length::Fixed(24.0)),
    ])
    .on_press(on_press_mesage);
    match collapsed {
        Collapsed::Collapsed => heading.into(),
        Collapsed::Expanded => {
            let children = two_column_text_widget(text);
            column![heading, children].into()
        }
    }
}

// TODO: See if I can return Widget instead of Element.
fn two_column_text_widget<'a>(
    text: impl Iterator<Item = (String, String)> + 'a,
) -> Element<'a, Message> {
    cosmic::widget::column::Column::with_children(text.map(|(col1, col2)| {
        cosmic::widget::flex_row(vec![
            cosmic::widget::text(col1).into(),
            cosmic::widget::text(col2).into(),
        ])
        .justify_content(JustifyContent::SpaceBetween)
        .into()
    }))
    .into()
}

/// (name, upgrade)
fn pretty_print_update(update: &Update) -> (String, String) {
    (
        update.pkgname.to_string(),
        format!(
            "{}-{}->{}-{}",
            update.pkgver_cur, update.pkgrel_cur, update.pkgver_new, update.pkgrel_new
        ),
    )
}

/// (name, upgrade)
fn pretty_print_devel_update(update: &DevelUpdate) -> (String, String) {
    (
        update.pkgname.to_string(),
        format!("{}->*{}*", update.pkgver_cur, update.ref_id_new,),
    )
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
        // .style(cosmic::theme::Svg::Custom(Rc::new(|theme| {
        //     cosmic::iced_style::svg::Appearance {
        //         color: Some(theme.cosmic().background.on.into()),
        //     }
        // })))
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
    let text = t(text.to_string()).font(cosmic::font::default());
    cosmic::widget::button::custom(
        cosmic::widget::layer_container(
            cosmic::widget::row::with_children(vec![icon, text.into()])
                .align_y(cosmic::iced::Alignment::Center)
                .spacing(2),
        )
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center)
        .height(Length::Fill),
    )
    // TODO: Decide what to do if vertical.
    .height(Length::Fixed(configured_height.get() as f32))
    // .style(Button::AppletIcon)
}
