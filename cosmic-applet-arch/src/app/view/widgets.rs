use super::AppIcon;
use crate::app::Message;
use crate::core::config::Config;
use crate::fl;
use crate::news::DatedNewsItem;
use arch_updates_rs::{AurUpdate, DevelUpdate, PacmanUpdate, SourceRepo};
use cosmic::iced::alignment::{Horizontal, Vertical};
use cosmic::iced::Length;
use cosmic::widget::{JustifyContent, Widget};
use cosmic::{theme, Element};
use std::collections::HashMap;

#[derive(Default, Copy, Clone)]
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

pub fn cosmic_body_text_row(text: String) -> Element<'static, Message> {
    cosmic::widget::container(
        cosmic::widget::text::body(text)
            .width(Length::Fill)
            .height(Length::Fixed(24.0))
            .align_y(Vertical::Center),
    )
    .padding(cosmic::applet::menu_control_padding())
    .into()
}

fn cosmic_collapsible_row_widget<'a>(
    contents: Element<'a, Message>,
    collapsed: Collapsed,
    title: String,
    on_press_mesage: Message,
) -> Element<'a, Message> {
    let icon_name = match collapsed {
        Collapsed::Collapsed => "go-down-symbolic",
        Collapsed::Expanded => "go-up-symbolic",
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
        Collapsed::Expanded => cosmic::iced_widget::column![heading, contents].into(),
    }
}

pub fn news_available_widget<'a>(
    news_list: impl ExactSizeIterator<Item = &'a DatedNewsItem> + 'a,
    icon: Option<AppIcon>,
    max_news_lines: usize,
) -> Element<'a, Message> {
    let cosmic::cosmic_theme::Spacing { space_xxs, .. } = theme::active().cosmic().spacing;
    match news_list.len() {
        0 => cosmic_body_text_row(fl!("no-news")),
        _ => {
            let news_header = |element: Element<'a, Message>| {
                cosmic::applet::menu_button(element).on_press(Message::ClearNewsMsg)
            };
            let news_header_text = cosmic::widget::text::body(fl!("news"));
            let news_header = match icon {
                Some(icon) => news_header(
                    cosmic::iced_widget::row![
                        cosmic::widget::icon::from_name(icon.to_str()),
                        news_header_text
                    ]
                    .spacing(space_xxs)
                    .into(),
                ),
                None => news_header(news_header_text.into()),
            };
            cosmic::iced_widget::column![
                news_header,
                news_list_widget(news_list, max_news_lines, space_xxs)
            ]
            .into()
        }
    }
}

pub fn updates_available_widget<'a>(
    package_list: impl ExactSizeIterator<Item = DisplayPackage> + 'a,
    collapsed: Collapsed,
    title: String,
    on_press_mesage: Message,
    max_items: usize,
) -> Element<'a, Message> {
    let cosmic::cosmic_theme::Spacing { space_xxs, .. } = theme::active().cosmic().spacing;
    let children = package_list_widget(package_list, max_items, space_xxs);
    cosmic_collapsible_row_widget(children, collapsed, title, on_press_mesage)
}

fn cosmic_overflowing_list_widget<'a>(
    items: impl ExactSizeIterator<Item = Element<'a, Message>>,
    max_items: usize,
    left_margin_px: u16,
) -> Element<'a, Message> {
    let list_len = items.len();
    let overflow_line = {
        if list_len > max_items {
            Some(fl!("n-more", n = (list_len - max_items)))
        } else {
            None
        }
    };
    let cosmic_padding = cosmic::applet::menu_control_padding();
    let footer_padding = cosmic_padding.left(cosmic_padding.left + left_margin_px as f32);
    let footer = overflow_line.map(|footer| {
        cosmic::widget::container(cosmic::widget::text::body(footer))
            .padding(footer_padding)
            .into()
    });
    cosmic::widget::column::Column::with_children(items.take(max_items).chain(footer)).into()
}

fn display_package_widget(
    package: DisplayPackage,
    left_margin_px: u16,
) -> Element<'static, Message> {
    let cosmic_padding = cosmic::applet::menu_control_padding();
    cosmic::widget::flex_row(vec![
        cosmic::widget::container(cosmic_url_widget_body(
            package.pretty_print_pkgname_and_repo(),
            package.url.clone(),
        ))
        .padding([0, 0, 0, left_margin_px])
        .into(),
        cosmic::widget::text::body(package.pretty_print_version_change()).into(),
    ])
    .justify_content(JustifyContent::SpaceBetween)
    .padding(cosmic_padding)
    .into()
}

fn display_news_widget(news: &DatedNewsItem, left_margin_px: u16) -> Element<'_, Message> {
    let cosmic_padding = cosmic::applet::menu_control_padding();
    cosmic::widget::flex_row(vec![
        cosmic::widget::container(cosmic_url_widget_body(
            news.title.clone().unwrap_or_default(),
            news.link.clone(),
        ))
        .padding([0, 0, 0, left_margin_px])
        .into(),
        cosmic::widget::text::body(news.date.format("%d/%m/%Y %H:%M").to_string()).into(),
    ])
    .justify_content(JustifyContent::SpaceBetween)
    .padding(cosmic_padding)
    .into()
}

fn package_list_widget<'a>(
    text: impl ExactSizeIterator<Item = DisplayPackage> + 'a,
    max_items: usize,
    left_margin_px: u16,
) -> Element<'a, Message> {
    cosmic_overflowing_list_widget(
        text.map(|pkg| display_package_widget(pkg, left_margin_px)),
        max_items,
        left_margin_px,
    )
}

pub fn news_list_widget<'a>(
    text: impl ExactSizeIterator<Item = &'a DatedNewsItem> + 'a,
    max_items: usize,
    left_margin_px: u16,
) -> Element<'a, Message> {
    cosmic_overflowing_list_widget(
        text.map(|pkg| display_news_widget(pkg, left_margin_px)),
        max_items,
        left_margin_px,
    )
}

// TODO: Underline if this is a URL
// Possibly blocked on https://github.com/iced-rs/iced/issues/2807
fn cosmic_url_widget_body(text: String, url: Option<String>) -> Element<'static, Message> {
    match url {
        Some(url) => cosmic::widget::tooltip(
            cosmic::iced::widget::mouse_area(cosmic::iced_widget::rich_text([
                cosmic::iced_widget::span(text).underline(true),
            ]))
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
    pub fn from_pacman_update(update: &PacmanUpdate, config: &Config) -> Self {
        Self {
            display_ver_new: format!("{}-{}", update.pkgver_new, update.pkgrel_new),
            display_ver_old: format!("{}-{}", update.pkgver_cur, update.pkgrel_cur),
            source_repo: update.source_repo.as_ref().map(ToString::to_string),
            pkgname: update.pkgname.to_string(),
            url: update.source_repo.clone().and_then(|source_repo| {
                pacman_url(&update.pkgname, source_repo, &config.other_repo_urls)
            }),
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

/// Get official Arch url for a package if it's in one of the official repos.
///
/// The `other_repo_urls` is a HashMap or unofficial repo names and urls that
/// the caller can provide. If its in an unofficial repo, and user has provided
/// a url, return the caller provided url with {pgkname} replaced with the
/// actual package name.
fn pacman_url(
    pkgname: &str,
    source_repo: SourceRepo,
    other_repo_urls: &HashMap<String, String>,
) -> Option<String> {
    if let SourceRepo::Other(other_repo_name) = source_repo {
        return other_repo_urls
            .get(&other_repo_name)
            .map(|url_raw| url_raw.replace("{pkgname}", pkgname));
    }
    // NOTE: the webpage will automatically redirect a url with architecture
    // `x86_64` to `any` if needed, so it's safe to hardcode x86_64 in the url for
    // now. Try this here: https://archlinux.org/packages/core/x86_64/pacman-mirrorlist/
    // TODO: add test for this.
    Some(format!(
        "https://archlinux.org/packages/{source_repo}/x86_64/{pkgname}/"
    ))
}

#[cfg(test)]
mod tests {
    use crate::app::view::widgets::pacman_url;

    #[tokio::test]
    async fn test_pacman_url_with_other_repo() {
        let other_repo_urls = [
            (
                "endeavouros".to_string(),
                "https://github.com/endeavouros-team/PKGBUILDS/tree/master/{pkgname}".to_string(),
            ),
            (
                "chaotic-aur".to_string(),
                "https://gitlab.com/chaotic-aur/pkgbuilds/-/tree/main/{pkgname}".to_string(),
            ),
        ]
        .into();
        let url = pacman_url(
            "cosmic-applet-arch",
            arch_updates_rs::SourceRepo::Other("chaotic-aur".into()),
            &other_repo_urls,
        );
        let url2 = pacman_url(
            "cosmic-applet-arch",
            arch_updates_rs::SourceRepo::Other("endeavouros".into()),
            &other_repo_urls,
        );
        assert_eq!(
            url.as_deref(),
            Some("https://gitlab.com/chaotic-aur/pkgbuilds/-/tree/main/cosmic-applet-arch")
        );
        assert_eq!(
            url2.as_deref(),
            Some("https://github.com/endeavouros-team/PKGBUILDS/tree/master/cosmic-applet-arch")
        );
    }
    #[tokio::test]
    async fn test_pacman_url_with_other_repo_no_pkgname() {
        let other_repo_urls = [(
            "endeavouros".to_string(),
            "https://github.com/endeavouros-team/PKGBUILDS/tree/master/".to_string(),
        )]
        .into();
        let url = pacman_url(
            "cosmic-applet-arch",
            arch_updates_rs::SourceRepo::Other("endeavouros".into()),
            &other_repo_urls,
        );
        assert_eq!(
            url.as_deref(),
            Some("https://github.com/endeavouros-team/PKGBUILDS/tree/master/")
        );
    }
    #[tokio::test]
    async fn test_pacman_url_with_other_repo_no_url() {
        let other_repo_urls = [(
            "endeavouros".to_string(),
            "https://github.com/endeavouros-team/PKGBUILDS/tree/master/".to_string(),
        )]
        .into();
        let url = pacman_url(
            "cosmic-applet-arch",
            arch_updates_rs::SourceRepo::Other("chaotic-aur".into()),
            &other_repo_urls,
        );
        assert_eq!(url.as_deref(), None);
    }
}
