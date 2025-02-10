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

fn cosmic_applet_divider(
    spacing: u16,
) -> impl Widget<Message, cosmic::Theme, cosmic::Renderer> + Into<Element<'static, Message>> {
    cosmic::applet::padded_control(cosmic::widget::divider::horizontal::default())
        .padding([0, spacing])
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

fn errors_row(error: impl Display) -> Element<'static, Message> {
    cosmic::widget::container(
        cosmic::widget::text::body(format!("Warning: {error}!!"))
            .width(Length::Fill)
            .height(Length::Fixed(24.0))
            .align_y(Vertical::Center),
    )
    .padding(cosmic::applet::menu_control_padding())
    .into()
}

fn collapsible_two_column_package_list_widget<'a>(
    // ((package name, package url), version change string)
    package_list: impl ExactSizeIterator<Item = ((String, String), String)> + 'a,
    collapsed: &Collapsed,
    title: String,
    on_press_mesage: Message,
    max_items: usize,
) -> Element<'a, Message> {
    let cosmic::cosmic_theme::Spacing { space_xxs, .. } = theme::active().cosmic().spacing;

    let icon_name = match collapsed {
        Collapsed::Collapsed => "go-down-symbolic",
        Collapsed::Expanded => "go-up-symbolic",
    };

    let list_len = package_list.len();

    let overflow_line = {
        if list_len > max_items {
            Some((fl!("n-more", n = (list_len - max_items)), "".to_string()))
        } else {
            None
        }
    };

    let heading = cosmic::applet::menu_button(cosmic::iced_widget::row![
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
            let children = two_column_package_list_widget(
                package_list.take(max_items).chain(overflow_line),
                space_xxs,
            );
            cosmic::iced_widget::column![heading, children].into()
        }
    }
}

// TODO: See if I can return Widget instead of Element.
fn two_column_package_list_widget<'a>(
    // ((package name, package url), version change string)
    text: impl Iterator<Item = (String, String)> + 'a,
    left_margin: u16,
) -> Element<'a, Message> {
    cosmic::widget::column::Column::with_children(text.map(|(col1, col2)| {
        cosmic::widget::button::custom(
            cosmic::widget::flex_row(vec![
                cosmic::widget::container(cosmic::widget::text::body(col1))
                    .padding([0, 0, 0, left_margin])
                    .into(),
                cosmic::widget::text::body(col2).into(),
            ])
            .justify_content(JustifyContent::SpaceBetween)
            .padding(cosmic::applet::menu_control_padding()),
        )
        .on_press(Message::OpenUrl("https://www.google.com".to_string()))
        .into()
    }))
    .into()
}

/// All the information required to display the package in the widget
struct DisplayPackage {
    display_ver_new: String,
    display_ver_old: String,
    url: String,
    pkgname: String,
}

impl DisplayPackage {
    fn pretty_print_version_change(&self) -> String {
        format!("{}->{}", self.display_ver_old, self.display_ver_new)
    }
    fn from_update(update: &Update) -> Self {
        Self {
            display_ver_new: format!("{}-{}", update.pkgver_new, update.pkgrel_new),
            display_ver_old: format!("{}-{}", update.pkgver_cur, update.pkgrel_cur),
            url: todo!(),
            pkgname: update.pkgname.to_string(),
        }
    }
    fn from_devel_update(update: &DevelUpdate) -> Self {
        Self {
            display_ver_new: format!("*{}*", update.ref_id_new),
            display_ver_old: format!("{}-{}", update.pkgver_cur, update.pkgrel_cur),
            url: todo!(),
            pkgname: update.pkgname.to_string(),
        }
    }
}

fn aur_url(pkgname: &str) -> String {}
fn pacman_url(pkgname: &str) -> String {}

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
