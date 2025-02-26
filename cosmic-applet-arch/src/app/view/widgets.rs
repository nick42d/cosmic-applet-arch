use super::Message;
use crate::fl;
use arch_updates_rs::{AurUpdate, DevelUpdate, PacmanUpdate, SourceRepo};
use cosmic::{
    iced::{
        alignment::{Horizontal, Vertical},
        Length,
    },
    theme,
    widget::{JustifyContent, Widget},
    Element,
};
use std::fmt::Display;

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

pub fn cosmic_applet_divider(
    spacing: u16,
) -> impl Widget<Message, cosmic::Theme, cosmic::Renderer> + Into<Element<'static, Message>> {
    cosmic::applet::padded_control(cosmic::widget::divider::horizontal::default())
        .padding([0, spacing])
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
        cosmic::widget::text::body(fl!("warning-error", error = error.to_string()))
            .width(Length::Fill)
            .height(Length::Fixed(24.0))
            .align_y(Vertical::Center),
    )
    .padding(cosmic::applet::menu_control_padding())
    .into()
}

pub fn collapsible_two_column_package_list_widget<'a>(
    package_list: impl ExactSizeIterator<Item = DisplayPackage> + 'a,
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
            Some(fl!("n-more", n = (list_len - max_items)))
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
                package_list.take(max_items),
                space_xxs,
                overflow_line,
            );
            cosmic::iced_widget::column![heading, children].into()
        }
    }
}

fn two_column_package_list_widget<'a>(
    text: impl Iterator<Item = DisplayPackage> + 'a,
    left_margin_px: u16,
    footer: Option<String>,
) -> Element<'a, Message> {
    let cosmic_padding = cosmic::applet::menu_control_padding();
    let footer_padding = cosmic_padding.left(cosmic_padding.left + left_margin_px as f32);
    let footer = footer.map(|footer| {
        cosmic::widget::container(cosmic::widget::text::body(footer))
            .padding(footer_padding)
            .into()
    });
    cosmic::widget::column::Column::with_children(
        text.map(|pkg| {
            cosmic::widget::flex_row(vec![
                cosmic::widget::container(cosmic_url_widget_body(
                    pkg.pretty_print_pkgname_and_repo(),
                    pkg.url.clone(),
                ))
                .padding([0, 0, 0, left_margin_px])
                .into(),
                cosmic::widget::text::body(pkg.pretty_print_version_change()).into(),
            ])
            .justify_content(JustifyContent::SpaceBetween)
            .padding(cosmic_padding)
            .into()
        })
        .chain(footer),
    )
    .into()
}

// TODO: Underline if this is a URL
// Possibly blocked on https://github.com/iced-rs/iced/issues/2807
fn cosmic_url_widget_body(text: String, url: Option<String>) -> Element<'static, Message> {
    match url {
        Some(url) => cosmic::widget::tooltip(
            cosmic::iced::widget::mouse_area(cosmic::iced_widget::text(text))
                .interaction(cosmic::iced::mouse::Interaction::Pointer)
                .on_press(Message::OpenUrl(url.clone())),
            cosmic::widget::text::body(url),
            cosmic::widget::tooltip::Position::FollowCursor,
        )
        .into(),
        None => cosmic::widget::text::body(text).into(),
    }
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
            Some(source_repo) => format!("{} ({})", self.pkgname, source_repo),
            None => self.pkgname.to_owned(),
        }
    }
    pub fn pretty_print_version_change(&self) -> String {
        format!("{}->{}", self.display_ver_old, self.display_ver_new)
    }
    pub fn from_pacman_update(update: &PacmanUpdate) -> Self {
        Self {
            display_ver_new: format!("{}-{}", update.pkgver_new, update.pkgrel_new),
            display_ver_old: format!("{}-{}", update.pkgver_cur, update.pkgrel_cur),
            source_repo: update.source_repo.as_ref().map(ToString::to_string),
            pkgname: update.pkgname.to_string(),
            url: update
                .source_repo
                .clone()
                .and_then(|source_repo| pacman_url(&update.pkgname, source_repo)),
        }
    }
    pub fn from_aur_update(update: &AurUpdate) -> Self {
        Self {
            display_ver_new: format!("{}-{}", update.pkgver_new, update.pkgrel_new),
            display_ver_old: format!("{}-{}", update.pkgver_cur, update.pkgrel_cur),
            source_repo: Some("aur".to_string()),
            pkgname: update.pkgname.to_string(),
            url: Some(aur_url(&update.pkgname)),
        }
    }
    pub fn from_devel_update(update: &DevelUpdate) -> Self {
        Self {
            display_ver_new: format!("*{}*", update.ref_id_new),
            display_ver_old: format!("{}-{}", update.pkgver_cur, update.pkgrel_cur),
            url: Some(aur_url(&update.pkgname)),
            pkgname: update.pkgname.to_string(),
            source_repo: Some("aur".to_string()),
        }
    }
}

/// Get AUR url for a package.
fn aur_url(pkgname: &str) -> String {
    format!("https://aur.archlinux.org/packages/{pkgname}")
}
/// Get official Arch url for a package, if it's in one of the official repos.
fn pacman_url(pkgname: &str, source_repo: SourceRepo) -> Option<String> {
    if let SourceRepo::Other(_) = source_repo {
        return None;
    }
    // NOTE: the webpage will automatically redirect a url with architecture
    // `x86_64` to `any` if needed, so it's safe to hardcode x86_64 in the url for
    // now. Try this here: https://archlinux.org/packages/core/x86_64/pacman-mirrorlist/
    // TODO: add test for this.
    Some(format!(
        "https://archlinux.org/packages/{source_repo}/x86_64/{pkgname}/"
    ))
}
