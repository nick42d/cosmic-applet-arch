use super::{CosmicAppletArch, Message};
use crate::fl;
use arch_updates_rs::{DevelUpdate, SourceRepo, Update};
use cosmic::{
    iced::{
        alignment::{Horizontal, Vertical},
        Length,
    },
    theme,
    widget::{JustifyContent, Widget},
    Element,
};
use std::{borrow::Cow, fmt::Display};

pub fn cosmic_applet_divider(
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

pub fn body_text_row(text: String) -> Element<'static, Message> {
    cosmic::widget::container(
        cosmic::widget::text::body(text)
            .width(Length::Fill)
            .height(Length::Fixed(24.0))
            .align_y(Vertical::Center),
    )
    .padding(cosmic::applet::menu_control_padding())
    .into()
}

pub fn errors_row(error: impl Display) -> Element<'static, Message> {
    cosmic::widget::container(
        cosmic::widget::text::body(format!("Warning: {error}!!"))
            .width(Length::Fill)
            .height(Length::Fixed(24.0))
            .align_y(Vertical::Center),
    )
    .padding(cosmic::applet::menu_control_padding())
    .into()
}

pub fn collapsible_two_column_package_list_widget<'a>(
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
    text: impl Iterator<Item = ((String, String), String)> + 'a,
    left_margin: u16,
) -> Element<'a, Message> {
    cosmic::widget::column::Column::with_children(text.map(|((col1, url), col2)| {
        cosmic::widget::flex_row(vec![
            cosmic::widget::container(cosmic_url_widget_body(col1, url))
                .padding([0, 0, 0, left_margin])
                .into(),
            cosmic::widget::text::body(col2).into(),
        ])
        .justify_content(JustifyContent::SpaceBetween)
        .padding(cosmic::applet::menu_control_padding())
        .into()
    }))
    .into()
}

fn cosmic_url_widget_body(text: String, url: String) -> Element<'static, Message> {
    cosmic::widget::button::custom(cosmic::widget::text::body(text))
        .on_press(Message::OpenUrl(url))
        .into()
}

/// All the information required to display the package in the widget
pub struct DisplayPackage {
    display_ver_new: String,
    display_ver_old: String,
    url: Option<String>,
    pkgname: String,
    source_repo: Option<String>,
}

impl DisplayPackage {
    pub fn pretty_print_pkgname_and_repo(&self) -> String {
        match &self.source_repo {
            Some(source_repo) => format!("{}/{}", source_repo, self.pkgname)
            None => self.pkgname.to_owned()
        }
    }
    pub fn pretty_print_version_change(&self) -> String {
        format!("{}->{}", self.display_ver_old, self.display_ver_new)
    }
    fn from_update(update: &Update) -> Self {
        Self {
            display_ver_new: format!("{}-{}", update.pkgver_new, update.pkgrel_new),
            display_ver_old: format!("{}-{}", update.pkgver_cur, update.pkgrel_cur),
            url: update
                .source_repo
                .and_then(|source_repo| package_url(&update.pkgname, source_repo)),
            pkgname: update.pkgname.to_string(),
            source_repo: update.source_repo.map(ToString::to_string),
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

fn package_url(pkgname: &str, source_repo: SourceRepo) -> Option<String> {
    match source_repo {
        SourceRepo::Aur => Some(aur_url(pkgname)),
        SourceRepo::Other(_) => None,
        other => Some(pacman_url(pkgname, other.to_string())),
    }
}
/// Get AUR url for a package.
fn aur_url(pkgname: &str) -> String {
    format!("https://aur.archlinux.org/packages/{pkgname}")
}
/// Get official Arch url for a package.
fn pacman_url(pkgname: &str, source_repo_string: String) -> String {
    // NOTE: the webpage will automatically redirect a url with architecture
    // `x86_64` to `any` if needed.
    // Try this here: https://archlinux.org/packages/core/x86_64/pacman-mirrorlist/
    // TODO: add test for this.
    format!("https://archlinux.org/packages/{source_repo_string}/x86_64/{pkgname}/")
}
