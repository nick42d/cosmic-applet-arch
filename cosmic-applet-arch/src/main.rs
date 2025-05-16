// SPDX-License-Identifier: GPL-3.0-only

/// The `app` module is used by convention to indicate the main component of our
/// application.
mod app;
mod core;
mod news;

use app::CosmicAppletArch;

fn main() -> cosmic::iced::Result {
    cosmic::applet::run::<CosmicAppletArch>(())
}
