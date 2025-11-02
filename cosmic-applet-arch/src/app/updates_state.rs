use crate::app::subscription::core::{
    ErrorVecWithHistory, OfflineUpdates, OnlineUpdates, UpdatesError,
};
use crate::app::Message;
use crate::core::config::UpdateType;
use arch_updates_rs::{AurUpdate, DevelUpdate, PacmanUpdate};
use chrono::Local;
use cosmic::app::Task;
use std::collections::HashSet;

#[derive(Default, Debug)]
pub enum UpdatesState {
    #[default]
    Init,
    Running {
        last_checked_online: chrono::DateTime<Local>,
        pacman: ErrorVecWithHistory<PacmanUpdate, UpdatesError>,
        aur: ErrorVecWithHistory<AurUpdate, UpdatesError>,
        devel: ErrorVecWithHistory<DevelUpdate, UpdatesError>,
        refreshing: bool,
    },
}

impl UpdatesState {
    /// Returns the total number of updates exluding the passed UpdateTypes.
    pub fn total_filtered(&self, exclude_from_count: &HashSet<UpdateType>) -> usize {
        let UpdatesState::Running {
            pacman, aur, devel, ..
        } = self
        else {
            return 0;
        };
        let total_updates = |u: UpdateType| match u {
            UpdateType::Aur => aur.len(),
            UpdateType::Devel => devel.len(),
            UpdateType::Pacman => pacman.len(),
        };
        HashSet::from([UpdateType::Aur, UpdateType::Devel, UpdateType::Pacman])
            .difference(exclude_from_count)
            .fold(0, |acc, e| acc + total_updates(*e))
    }
    pub fn has_errors(&self) -> bool {
        match self {
            UpdatesState::Init => false,
            UpdatesState::Running {
                pacman, aur, devel, ..
            } => {
                !(matches!(pacman, ErrorVecWithHistory::Ok { .. })
                    && matches!(aur, ErrorVecWithHistory::Ok { .. })
                    && matches!(devel, ErrorVecWithHistory::Ok { .. }))
            }
        }
    }
    pub fn get_refreshing(&self) -> bool {
        match self {
            UpdatesState::Init => true,
            UpdatesState::Running { refreshing, .. } => *refreshing,
        }
    }
    pub fn set_refreshing(&mut self) {
        let prev_state = std::mem::take(self);
        if let UpdatesState::Running {
            last_checked_online,
            pacman,
            aur,
            devel,
            ..
        } = prev_state
        {
            *self = UpdatesState::Running {
                last_checked_online,
                pacman,
                aur,
                devel,
                refreshing: true,
            }
        }
    }
    pub fn handle_online_updates(
        &mut self,
        updates: OnlineUpdates,
        update_time: chrono::DateTime<Local>,
    ) -> Task<Message> {
        // When first receiving updates, autosize will not trigger until the second
        // message is received. So, we intentionally bounce this message if it's
        // the first time updates have been received.
        let task = if matches!(self, UpdatesState::Init) {
            Task::done(
                Message::RefreshedUpdatesOnline {
                    updates: updates.clone(),
                    update_time,
                }
                .into(),
            )
        } else {
            Task::none()
        };
        let OnlineUpdates { pacman, aur, devel } = updates;
        let prev_state = std::mem::take(self);
        *self = match prev_state {
            UpdatesState::Init => UpdatesState::Running {
                last_checked_online: update_time,
                pacman: ErrorVecWithHistory::new_from_result(pacman),
                aur: ErrorVecWithHistory::new_from_result(aur),
                devel: ErrorVecWithHistory::new_from_result(devel),
                refreshing: false,
            },
            UpdatesState::Running {
                pacman: prev_pacman,
                aur: prev_aur,
                devel: prev_devel,
                ..
            } => UpdatesState::Running {
                last_checked_online: update_time,
                pacman: prev_pacman.replace_with_result_preserving_history(pacman),
                aur: prev_aur.replace_with_result_preserving_history(aur),
                devel: prev_devel.replace_with_result_preserving_history(devel),
                refreshing: false,
            },
        };
        task
    }
    pub fn handle_offline_updates(&mut self, updates: OfflineUpdates) -> Task<Message> {
        let OfflineUpdates { pacman, aur, devel } = updates;
        let prev_state = std::mem::take(self);
        *self = match prev_state {
            UpdatesState::Init => {
                eprintln!(
                "WARNING: Offline update received by UI before it had received an online update"
            );
                prev_state
            }
            UpdatesState::Running {
                pacman: prev_pacman,
                aur: prev_aur,
                devel: prev_devel,
                last_checked_online,
                refreshing,
            } => UpdatesState::Running {
                last_checked_online,
                pacman: prev_pacman.replace_with_option_result_preserving_history(pacman),
                aur: prev_aur.replace_with_option_result_preserving_history(aur),
                devel: prev_devel.replace_with_option_result_preserving_history(devel),
                refreshing,
            },
        };
        Task::none()
    }
}
