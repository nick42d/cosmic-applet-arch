use crate::app::subscription::core::BasicResultWithHistory;
use crate::app::view::{
    cosmic_applet_divider, cosmic_body_text_row, news_available_widget, updates_available_widget,
    AppIcon, Collapsed, DisplayPackage, MAX_NEWS_LINES, MAX_UPDATE_LINES,
};
use crate::app::{CollapsibleType, Message, NewsState, UpdatesState};
use crate::{fl, CosmicAppletArch};
use arch_updates_rs::{AurUpdate, DevelUpdate, PacmanUpdate};
use chrono::{DateTime, Local};
use cosmic::{theme, Element};

/// All required data to render the news-related parts of applet popup
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
        NewsState::InitError { .. } => NewsView::ErrorOnly,
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

/// All required data to render the updates-related parts of applet popup
enum UpdatesView<'a> {
    Loading,
    Loaded {
        pacman_updates: SourceUpdatesView<'a, PacmanUpdate>,
        aur_updates: SourceUpdatesView<'a, AurUpdate>,
        devel_updates: SourceUpdatesView<'a, DevelUpdate>,
        last_refreshed: chrono::DateTime<Local>,
        refreshing: bool,
        no_updates_available: bool,
    },
}

fn get_updates_view(app: &CosmicAppletArch) -> UpdatesView<'_> {
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
        pacman_updates: get_source_updates_view(pacman),
        aur_updates: get_source_updates_view(aur),
        devel_updates: get_source_updates_view(devel),
        last_refreshed: last_checked_online,
        refreshing,
        no_updates_available,
    }
}

/// All required data to render the source-specifc updates-related parts of
/// applet popup
enum SourceUpdatesView<'a, T> {
    ErrorOnly,
    Updates {
        updates: &'a Vec<T>,
        has_error: bool,
    },
}

fn get_source_updates_view<T>(
    updates: &BasicResultWithHistory<Vec<T>>,
) -> SourceUpdatesView<'_, T> {
    match updates {
        BasicResultWithHistory::Error => SourceUpdatesView::ErrorOnly,
        BasicResultWithHistory::Ok { value } => SourceUpdatesView::Updates {
            updates: value,
            has_error: false,
        },
        BasicResultWithHistory::ErrorWithHistory { last_value, .. } => SourceUpdatesView::Updates {
            updates: last_value,
            has_error: true,
        },
    }
}

// view_window is what is displayed in the popup.
pub fn view_window(app: &CosmicAppletArch, _id: cosmic::iced::window::Id) -> Element<'_, Message> {
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
        updates: &SourceUpdatesView<'a, T>,
        converter: impl Fn(&T) -> DisplayPackage + 'a,
        collapsed: Collapsed,
        collapsible_type: crate::app::CollapsibleType,
    ) -> Option<Element<'a, Message>> {
        let row = match updates {
            SourceUpdatesView::Updates {
                updates,
                has_error: false,
            } if updates.is_empty() => {
                return None;
            }
            SourceUpdatesView::ErrorOnly => {
                cosmic_body_text_row(fl!("error-checking-updates", updateSource = source_name))
            }
            SourceUpdatesView::Updates {
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
            SourceUpdatesView::Updates {
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
