// SPDX-License-Identifier: GPL-3.0-only

/// The `app` module is used by convention to indicate the main component of our
/// application.
mod app;
mod core;
mod news;

use app::CosmicAppletArch;

fn main() -> cosmic::iced::Result {
    core::localization::localize();
    let rt =
        tokio::runtime::Runtime::new().expect("Expected to be able to initiate a tokio runtime");
    let config = rt.block_on(core::config::get_config()).unwrap();
    cosmic::applet::run::<CosmicAppletArch>(config)
}
