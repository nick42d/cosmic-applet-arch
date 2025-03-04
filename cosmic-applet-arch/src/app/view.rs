use super::{CosmicAppletArch, Message, UpdatesState};
use crate::{app::NewsState, fl};
use chrono::{DateTime, Local};
use cosmic::{
    app::Core,
    iced::{
        alignment::{Horizontal, Vertical},
        Length,
    },
    theme::{self, Button},
    widget::Id,
    Application, Element,
};
use std::borrow::Cow;
use std::{rc::Rc, sync::LazyLock};

pub use widgets::*;
/// What is displayed in the taskbar
pub mod view;
/// What is display when opening the applet menu
pub mod view_window;
mod widgets;

const MAX_UPDATE_LINES: usize = 20;
const MAX_NEWS_LINES: usize = 3;

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
