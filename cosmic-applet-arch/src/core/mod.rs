// SPDX-License-Identifier: GPL-3.0-only

use directories::ProjectDirs;
pub mod localization;

pub fn proj_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("com", "nick42d", "cosmic-applet-arch")
}
