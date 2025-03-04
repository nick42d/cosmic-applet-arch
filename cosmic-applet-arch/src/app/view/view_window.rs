use super::{CosmicAppletArch, Message, UpdatesState};
use crate::{
    app::{
        view::{
            body_text_row, cosmic_applet_divider, errors_row_widget, news_list_widget,
            updates_available_widget, AppIcon, DisplayPackage, MAX_NEWS_LINES, MAX_UPDATE_LINES,
        },
        NewsState,
    },
    fl,
};
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
        | UpdatesState::Refreshing {
            last_checked_online,
            ..
        }
        | UpdatesState::Error {
            last_checked_online,
            ..
        } => Some(
            cosmic::applet::menu_button(cosmic::widget::text::body(last_checked_string(
                last_checked_online,
            )))
            .on_press(Message::ForceGetUpdates),
        ),
    };
    let loading_row = if matches!(
        app.updates,
        UpdatesState::Init | UpdatesState::InitError { .. }
    ) {
        Some(body_text_row(fl!("loading")))
    } else {
        None
    };
    let errors_row = match &app.updates {
        UpdatesState::InitError { error } => Some(errors_row_widget(error)),
        // TODO: This should be a special case where the error is pressable.
        UpdatesState::Error { error, .. } => Some(errors_row_widget(error)),
        UpdatesState::Init | UpdatesState::Received { .. } | UpdatesState::Refreshing { .. } => {
            None
        }
    };

    let updates = match &app.updates {
        UpdatesState::Received { value, .. } => value,
        UpdatesState::Refreshing { last_value, .. } => last_value,
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
            .map(DisplayPackage::from_pacman_update),
        &app.pacman_list_state,
        fl!(
            "updates-available",
            numberUpdates = pm,
            updateSource = "pacman"
        ),
        Message::ToggleCollapsible(crate::app::CollapsibleType::PacmanUpdates),
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
        Message::ToggleCollapsible(crate::app::CollapsibleType::AurUpdates),
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
        Message::ToggleCollapsible(crate::app::CollapsibleType::DevelUpdates),
        MAX_UPDATE_LINES,
    );
    let mut news_error_row = None;
    todo!("News row should be different, if there isn't any news!");
    let news_row = match &app.news {
        NewsState::Init => None,
        NewsState::InitError { error } => Some(errors_row_widget(error)),
        NewsState::Received {
            last_checked_online,
            value,
        } => Some(
            cosmic::iced_widget::column![
                cosmic::applet::menu_button(cosmic::widget::text::body(fl!("news")))
                    .on_press(Message::ClearNewsMsg),
                news_list_widget(value.iter(), MAX_NEWS_LINES, space_xxs)
            ]
            .into(),
        ),
        NewsState::Clearing {
            last_value,
            last_checked_online,
        } => Some(
            cosmic::iced_widget::column![
                cosmic::applet::menu_button(cosmic::iced_widget::row![
                    cosmic::widget::icon::from_name(AppIcon::Loading.to_str()),
                    cosmic::widget::text::body(fl!("news"))
                ])
                .on_press(Message::ClearNewsMsg),
                news_list_widget(last_value.iter(), MAX_NEWS_LINES, space_xxs)
            ]
            .into(),
        ),
        NewsState::ClearingError {
            last_value,
            last_checked_online,
        } => Some(
            cosmic::iced_widget::column![
                cosmic::applet::menu_button(cosmic::iced_widget::row![
                    cosmic::widget::icon::from_name(AppIcon::Error.to_str()),
                    cosmic::widget::text::body(fl!("news"))
                ])
                .on_press(Message::ClearNewsMsg),
                news_list_widget(last_value.iter(), MAX_NEWS_LINES, space_xxs)
            ]
            .into(),
        ),
        NewsState::Error {
            last_value,
            error,
            last_checked_online,
        } => {
            news_error_row = Some(errors_row_widget(error));
            Some(
                cosmic::iced_widget::column![
                    cosmic::applet::menu_button(cosmic::widget::text::body(fl!("news")))
                        .on_press(Message::ClearNewsMsg),
                    news_list_widget(last_value.iter(), MAX_NEWS_LINES, space_xxs)
                ]
                .into(),
            )
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
        .push_maybe((total_updates == 0).then_some(body_text_row(fl!("no-updates-available"))))
        .push(cosmic_applet_divider(space_s).into())
        .push_maybe(last_checked_row)
        .push_maybe(loading_row)
        .push_maybe(errors_row)
        .push_maybe(news_divider)
        .push_maybe(news_row)
        .push_maybe(news_error_row);
    app.core.applet.popup_container(content_list).into()
}
