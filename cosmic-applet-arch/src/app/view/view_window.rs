use crate::app::view::{
    cosmic_applet_divider, cosmic_body_text_row, errors_row_widget, news_available_widget,
    updates_available_widget, AppIcon, DisplayPackage, MAX_NEWS_LINES, MAX_UPDATE_LINES,
};
use crate::app::{Message, NewsState, UpdatesState};
use crate::{fl, CosmicAppletArch};
use chrono::{DateTime, Local};
use cosmic::{theme, Element};

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

    let last_checked_row = match app.updates {
        UpdatesState::Init | UpdatesState::InitError { .. } => None,
        UpdatesState::Received {
            last_checked_online,
            ..
        }
        | UpdatesState::Error {
            last_checked_online,
            ..
        } => {
            let last_checked_text_widget =
                cosmic::widget::text::body(last_checked_string(last_checked_online));
            let last_checked_widget = if app.updates_refreshing {
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
    let loading_row = if matches!(
        app.updates,
        UpdatesState::Init | UpdatesState::InitError { .. }
    ) {
        Some(cosmic_body_text_row(fl!("loading")))
    } else {
        None
    };
    let errors_row = match &app.updates {
        UpdatesState::InitError { error } => Some(errors_row_widget(error)),
        // NOTE: This could be a special case where the error is pressable.
        UpdatesState::Error { error, .. } => Some(errors_row_widget(error)),
        UpdatesState::Init | UpdatesState::Received { .. } => None,
    };

    let updates = match &app.updates {
        UpdatesState::Received { value, .. } => value,
        UpdatesState::Error { last_value, .. } => last_value,
        UpdatesState::Init | UpdatesState::InitError { .. } => {
            let content_list = content_list
                .push_maybe(last_checked_row)
                .push_maybe(loading_row)
                .push_maybe(errors_row);
            return app.core.applet.popup_container(content_list).into();
        }
    };

    let pm = updates.pacman.len();
    let aur = updates.aur.len();
    let dev = updates.devel.len();

    let pacman_list = updates_available_widget(
        updates
            .pacman
            .iter()
            .map(|pkg| DisplayPackage::from_pacman_update(pkg, &app.config)),
        &app.pacman_list_state,
        fl!(
            "updates-available",
            numberUpdates = pm,
            updateSource = "pacman"
        ),
        Message::ToggleCollapsible(crate::app::CollapsibleType::Pacman),
        MAX_UPDATE_LINES,
    );
    let aur_list = updates_available_widget(
        updates.aur.iter().map(DisplayPackage::from_aur_update),
        &app.aur_list_state,
        fl!(
            "updates-available",
            numberUpdates = aur,
            updateSource = "AUR"
        ),
        Message::ToggleCollapsible(crate::app::CollapsibleType::Aur),
        MAX_UPDATE_LINES,
    );
    let devel_list = updates_available_widget(
        updates.devel.iter().map(DisplayPackage::from_devel_update),
        &app.devel_list_state,
        fl!(
            "updates-available",
            numberUpdates = dev,
            updateSource = "devel"
        ),
        Message::ToggleCollapsible(crate::app::CollapsibleType::Devel),
        MAX_UPDATE_LINES,
    );
    let mut news_error_row = None;
    let news_row = match &app.news {
        NewsState::Init => None,
        NewsState::InitError { error } => Some(errors_row_widget(error)),
        NewsState::Received { value, .. } => {
            Some(news_available_widget(value.iter(), None, MAX_NEWS_LINES))
        }
        NewsState::Clearing { last_value, .. } => Some(news_available_widget(
            last_value.iter(),
            Some(AppIcon::Loading),
            MAX_NEWS_LINES,
        )),
        NewsState::ClearingError { last_value, .. } => Some(news_available_widget(
            last_value.iter(),
            Some(AppIcon::Error),
            MAX_NEWS_LINES,
        )),
        NewsState::Error {
            last_value, error, ..
        } => {
            news_error_row = Some(errors_row_widget(error));
            Some(news_available_widget(
                last_value.iter(),
                None,
                MAX_NEWS_LINES,
            ))
        }
    };
    let news_divider =
        if last_checked_row.is_some() || loading_row.is_some() || errors_row.is_some() {
            Some(cosmic_applet_divider(space_s).into())
        } else {
            None
        };

    let total_updates = pm + aur + dev;
    let content_list = content_list
        .push_maybe((pm > 0).then_some(pacman_list))
        .push_maybe((aur > 0 && pm > 0).then_some(cosmic_applet_divider(space_s).into()))
        .push_maybe((aur > 0).then_some(aur_list))
        .push_maybe((dev > 0 && pm + aur > 0).then_some(cosmic_applet_divider(space_s).into()))
        .push_maybe((dev > 0).then_some(devel_list))
        .push_maybe(
            (total_updates == 0).then_some(cosmic_body_text_row(fl!("no-updates-available"))),
        )
        .push(cosmic_applet_divider(space_s).into())
        .push_maybe(last_checked_row)
        .push_maybe(loading_row)
        .push_maybe(errors_row)
        .push_maybe(news_divider)
        .push_maybe(news_row)
        .push_maybe(news_error_row);
    app.core.applet.popup_container(content_list).into()
}
