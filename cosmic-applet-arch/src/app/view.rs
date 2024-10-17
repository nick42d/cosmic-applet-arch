use super::{CosmicAppletArch, Message};
use arch_updates_rs::{DevelUpdate, Update};
use cosmic::{
    app::Core,
    applet::{cosmic_panel_config::PanelSize, Size},
    iced::{
        alignment::{Horizontal, Vertical},
        widget,
        window::Id,
        Length,
    },
    iced_widget::row,
    theme::Button,
    widget::{flex_row, settings, JustifyContent},
    Application, Element,
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
    let pm = app.updates.pacman.len();
    let au = app.updates.aur.len();
    let dev = app.updates.devel.len();

    let total_updates = pm + au + dev;

    if total_updates > 0 {
        applet_button_with_text(
            app.core(),
            AppIcon::UpdatesAvailable.to_str(),
            format!("{pm}/{au}/{dev}"),
        )
        .on_press_down(Message::TogglePopup)
        .into()
    } else {
        app.core
            .applet
            .icon_button(AppIcon::UpToDate.to_str())
            .on_press(Message::TogglePopup)
            .into()
    }
}

fn pluralise_n_updates(n: usize) -> String {
    match n {
        1 => "1 update".to_owned(),
        n => format!("{n} updates"),
    }
}

// view_window is what is displayed in the popup.
pub fn view_window(app: &CosmicAppletArch, _id: Id) -> Element<Message> {
    let content_list = cosmic::widget::list_column().padding(5).spacing(0);
    const MAX_LINES: usize = 5;

    let pm = app.updates.pacman.len();
    let au = app.updates.aur.len();
    let dev = app.updates.devel.len();

    let chain = |n: usize| if n > 5 { Some("...".to_string()) } else { None };

    let pacman_list = cosmic::widget::column::Column::with_children(
        app.updates
            .pacman
            .iter()
            .map(pretty_print_update)
            .map(|(name, update)| {
                cosmic::widget::flex_row(vec![
                    cosmic::widget::text(name).into(),
                    cosmic::widget::text(update).into(),
                ])
                .justify_content(JustifyContent::SpaceBetween)
                .into()
            }),
    );

    let aur_list = cosmic::widget::column::Column::with_children(
        app.updates
            .aur
            .iter()
            .map(pretty_print_update)
            .map(|(name, update)| {
                cosmic::widget::flex_row(vec![
                    cosmic::widget::text(name).into(),
                    cosmic::widget::text(update).into(),
                ])
                .justify_content(JustifyContent::SpaceBetween)
                .into()
            }),
    );

    let last_checked = match app.last_checked {
        Some(t) => OffsetDateTime::format(
            t.into(),
            &time::format_description::parse("[day]/[month] [hour]:[minute]").unwrap(),
        )
        .unwrap(),
        None => "Not yet".to_owned(),
    };

    let total_updates = pm + au + dev;
    let content_list = if total_updates > 0 {
        content_list
            .add(cosmic::widget::text(format!("{pm} Pacman updates:")))
            .add(pacman_list)
            .add(cosmic::widget::text(format!("{au} AUR updates:")))
            .add(aur_list)
            .add(cosmic::widget::text(format!("{dev} Dev updates: todo")))
            .add(cosmic::widget::text(format!(
                "Last checked: {last_checked}"
            )))
            .add(
                cosmic::widget::button::custom(cosmic::widget::text("Click to refresh"))
                    .on_press(Message::ForceGetUpdates),
            )
            .add(cosmic::widget::warning("Warning!"))
    } else {
        content_list.add(cosmic::widget::text("No updates available"))
    };
    app.core.applet.popup_container(content_list).into()
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
