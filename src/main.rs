#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use windows::Win32::UI::WindowsAndMessaging::MB_OK;

mod app;
mod cpu_usage;
mod events;
mod icon_manager;
mod settings;
mod windows_api;

use crate::{
    app::App,
    icon_manager::IconManager,
    settings::{get_current_icon, get_current_theme, migrate_legacy_settings},
    windows_api::safe_message_box,
};

fn main() {
    // Migrate legacy settings if needed
    migrate_legacy_settings();

    // Load icons
    let icon_manager = IconManager::load_icons().expect("Failed to load icons");

    // Get current icon and theme
    let icon_name = get_current_icon();
    let theme = get_current_theme();

    std::panic::set_hook(Box::new(|e| {
        let msg = format!("Panic: {}", e);
        if let Err(err) = safe_message_box(&msg, "RustCat Error", MB_OK.0) {
            eprintln!("Failed to show panic dialog: {}", err);
        }
    }));

    let app = App::new(icon_manager, &icon_name, Some(theme)).expect("Failed to create app");

    app.start_animation_thread();

    app.run_message_loop();

    app.shutdown();
}
