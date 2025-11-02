use crate::app::subscription::core::{ErrorVecWithHistory, UpdatesError};
use crate::app::view::{
    cosmic_applet_divider, cosmic_body_text_row, news_available_widget, updates_available_widget,
    AppIcon, Collapsed, DisplayPackage, MAX_NEWS_LINES, MAX_UPDATE_LINES,
};
use crate::app::{CollapsibleType, Message, NewsState, UpdatesState};
use crate::{fl, CosmicAppletArch};
use arch_updates_rs::{AurUpdate, DevelUpdate, PacmanUpdate};
use chrono::{DateTime, Local};
use cosmic::{theme, Element};

enum NewsView<'a> {
    Empty,
    ErrorOnly,
    News {
        icon: Option<AppIcon>,
        news: &'a Vec<crate::news::DatedNewsItem>,
        has_error: bool,
    },
}

fn get_news_view(app: &CosmicAppletArch) -> NewsView<'_> {
    match &app.news {
        NewsState::Init => NewsView::Empty,
        NewsState::InitError { .. } => NewsView::Empty,
        NewsState::Received { value: news, .. } => NewsView::News {
            icon: None,
            news,
            has_error: false,
        },
        NewsState::Clearing {
            last_value: news, ..
        } => NewsView::News {
            icon: Some(AppIcon::Loading),
            news,
            has_error: false,
        },
        NewsState::ClearingError {
            last_value: news, ..
        } => NewsView::News {
            icon: Some(AppIcon::Error),
            news,
            has_error: false,
        },
        NewsState::Error {
            last_value: news, ..
        } => NewsView::News {
            icon: None,
            news,
            has_error: true,
        },
    }
}

enum UpdateView<'a, T> {
    ErrorOnly,
    Updates {
        updates: &'a Vec<T>,
        has_error: bool,
    },
}

fn get_update_view<T, E>(updates: &ErrorVecWithHistory<T, E>) -> UpdateView<'_, T> {
    match updates {
        ErrorVecWithHistory::Error { .. } => UpdateView::ErrorOnly,
        ErrorVecWithHistory::Ok { value } => UpdateView::Updates {
            updates: value,
            has_error: false,
        },
        ErrorVecWithHistory::ErrorWithHistory { last_value, .. } => UpdateView::Updates {
            updates: last_value,
            has_error: true,
        },
    }
}

enum UpdatesView<'a> {
    Loading,
    Loaded {
        pacman_updates: UpdateView<'a, PacmanUpdate>,
        aur_updates: UpdateView<'a, AurUpdate>,
        devel_updates: UpdateView<'a, DevelUpdate>,
        last_refreshed: chrono::DateTime<Local>,
        refreshing: bool,
        no_updates_available: bool,
    },
}

pub fn get_updates_view(app: &CosmicAppletArch) -> UpdatesView<'_> {
    let UpdatesState::Running {
        last_checked_online,
        ref pacman,
        ref aur,
        ref devel,
        refreshing,
    } = app.updates
    else {
        return UpdatesView::Loading;
    };
    let no_updates_available = pacman.len() == 0
        && !pacman.has_error()
        && aur.len() == 0
        && !aur.has_error()
        && devel.len() == 0
        && !devel.has_error();
    UpdatesView::Loaded {
        pacman_updates: get_update_view(&pacman),
        aur_updates: get_update_view(&aur),
        devel_updates: get_update_view(&devel),
        last_refreshed: last_checked_online,
        refreshing,
        no_updates_available,
    }
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

    let news_view = get_news_view(app);
    let updates_view = get_updates_view(app);

    let (news_row, news_error_row) = match news_view {
        NewsView::Empty => (None, None),
        NewsView::ErrorOnly => (None, Some(cosmic_body_text_row(fl!("error-checking-news")))),
        NewsView::News {
            icon,
            news,
            has_error: false,
        } => (
            Some(news_available_widget(news.iter(), icon, MAX_NEWS_LINES)),
            None,
        ),
        NewsView::News {
            icon,
            news,
            has_error: true,
        } => (
            Some(news_available_widget(news.iter(), icon, MAX_NEWS_LINES)),
            Some(cosmic_body_text_row(fl!("error-checking-news"))),
        ),
    };

    let last_checked_row = match updates_view {
        UpdatesView::Loading => None,
        UpdatesView::Loaded {
            last_refreshed,
            refreshing,
            ..
        } => {
            let last_checked_text_widget =
                cosmic::widget::text::body(last_checked_string(last_refreshed));
            let last_checked_widget = if refreshing {
                let row = cosmic::widget::row()
                    .spacing(space_xxs)
                    .push(cosmic::widget::icon(
                        cosmic::widget::icon::from_name("emblem-synchronizing-symbolic").handle(),
                    ))
                    .push(last_checked_text_widget);
                cosmic::applet::menu_button(row).on_press(Message::ForceGetUpdates)
            } else {
                cosmic::applet::menu_button(last_checked_text_widget)
                    .on_press(Message::ForceGetUpdates)
            };
            Some(last_checked_widget)
        }
    };

    let loading_row = if matches!(updates_view, UpdatesView::Loading) {
        Some(cosmic_body_text_row(fl!("loading")))
    } else {
        None
    };

    fn get_row_for_source<'a, T>(
        source_name: &'static str,
        updates: &UpdateView<'a, T>,
        converter: impl Fn(&T) -> DisplayPackage + 'a,
        collapsed: Collapsed,
        collapsible_type: crate::app::CollapsibleType,
    ) -> Option<Element<'a, Message>> {
        let row = match updates {
            UpdateView::Updates {
                updates,
                has_error: false,
            } if updates.is_empty() => {
                return None;
            }
            UpdateView::ErrorOnly => {
                cosmic_body_text_row(fl!("error-checking-updates", updateSource = source_name))
            }
            UpdateView::Updates {
                updates,
                has_error: false,
            } => updates_available_widget(
                updates.iter().map(converter),
                collapsed,
                fl!(
                    "updates-available",
                    numberUpdates = updates.len(),
                    updateSource = source_name
                ),
                Message::ToggleCollapsible(collapsible_type),
                MAX_UPDATE_LINES,
            ),
            UpdateView::Updates {
                updates,
                has_error: true,
            } => updates_available_widget(
                updates.iter().map(converter),
                collapsed,
                fl!(
                    "updates-available-with-error",
                    numberUpdates = updates.len(),
                    updateSource = source_name
                ),
                Message::ToggleCollapsible(collapsible_type),
                MAX_UPDATE_LINES,
            ),
        };
        Some(row)
    }

    let pacman_row = match &updates_view {
        UpdatesView::Loading => None,
        UpdatesView::Loaded { pacman_updates, .. } => get_row_for_source(
            "pacman",
            pacman_updates,
            |update| DisplayPackage::from_pacman_update(update, &app.config),
            app.pacman_list_state,
            CollapsibleType::Pacman,
        ),
    };
    let pacman_row_divider = if pacman_row.is_some() {
        Some(cosmic_applet_divider(space_s).into())
    } else {
        None
    };
    let aur_row = match &updates_view {
        UpdatesView::Loading => None,
        UpdatesView::Loaded { aur_updates, .. } => get_row_for_source(
            "AUR",
            aur_updates,
            DisplayPackage::from_aur_update,
            app.aur_list_state,
            CollapsibleType::Aur,
        ),
    };
    let aur_row_divider = if aur_row.is_some() {
        Some(cosmic_applet_divider(space_s).into())
    } else {
        None
    };
    let devel_row = match &updates_view {
        UpdatesView::Loading => None,
        UpdatesView::Loaded { devel_updates, .. } => get_row_for_source(
            "devel",
            devel_updates,
            DisplayPackage::from_devel_update,
            app.devel_list_state,
            CollapsibleType::Devel,
        ),
    };
    let devel_row_divider = if devel_row.is_some() {
        Some(cosmic_applet_divider(space_s).into())
    } else {
        None
    };

    let no_updates_available_row = if matches!(
        &updates_view,
        UpdatesView::Loaded {
            no_updates_available: true,
            ..
        }
    ) {
        Some(cosmic_body_text_row(fl!("no-updates-available")))
    } else {
        None
    };

    let content_list = content_list
        .push_maybe(pacman_row)
        .push_maybe(pacman_row_divider)
        .push_maybe(aur_row)
        .push_maybe(aur_row_divider)
        .push_maybe(devel_row)
        .push_maybe(devel_row_divider)
        .push_maybe(no_updates_available_row)
        .push_maybe(last_checked_row)
        .push_maybe(loading_row)
        .push(cosmic_applet_divider(space_s).into())
        .push_maybe(news_row)
        .push_maybe(news_error_row);
    app.core
        .applet
        .popup_container(content_list)
        .limits(
            cosmic::iced::Limits::NONE
                .min_height(200.)
                .min_width(300.0)
                .max_width(500.0)
                .max_height(1080.0),
        )
        .into()
}
