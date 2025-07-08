#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

mod app;
mod events;
mod icon_manager;
mod platform;

use crate::{
    icon_manager::IconManager,
    platform::{SettingsManager, SettingsManagerImpl, SystemIntegration, SystemIntegrationImpl},
};

#[cfg(target_os = "macos")]
use crate::platform::macos::app::MacosApp;
#[cfg(windows)]
use crate::platform::windows::app::WindowsApp;

fn main() {
    // Migrate legacy settings if needed
    SettingsManagerImpl::migrate_legacy_settings();

    // Load icons
    let icon_manager = IconManager::load_icons().expect("Failed to load icons");

    // Get current icon and theme
    let icon_name = SettingsManagerImpl::get_current_icon();
    let theme = SettingsManagerImpl::get_current_theme();

    std::panic::set_hook(Box::new(|e| {
        let msg = format!("Panic: {}", e);
        if let Err(err) = SystemIntegrationImpl::show_dialog(&msg, "RustCat Error") {
            eprintln!("Failed to show panic dialog: {}", err);
        }
    }));

    #[cfg(target_os = "windows")]
    let app = WindowsApp::new(icon_manager, &icon_name, Some(theme)).expect("Failed to create app");
    #[cfg(target_os = "macos")]
    let app = MacosApp::new(icon_manager, &icon_name, Some(theme)).expect("Failed to create app");

    app.start_animation_thread();

    app.run();
}
