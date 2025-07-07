#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use windows::Win32::UI::WindowsAndMessaging::MB_OK;

mod cpu_usage;
mod events;
mod settings;
mod windows_api;
mod app;
mod icon_manager;

#[allow(dead_code)]
mod icon_data {
    include!(concat!(env!("OUT_DIR"), "/icons.rs"));
}

use crate::{
    app::App,
    icon_manager::IconManager,
    settings::get_icon_id,
    windows_api::safe_message_box,
};



fn main() {
    let icon_id = get_icon_id();
    let icon_manager = IconManager::load_icons().expect("Failed to load icons");

    std::panic::set_hook(Box::new(|e| {
        let msg = format!("Panic: {}", e);
        if let Err(err) = safe_message_box(&msg, "RustCat Error", MB_OK.0) {
            eprintln!("Failed to show panic dialog: {}", err);
        }
    }));

    let app = App::new(icon_manager, icon_id).expect("Failed to create app");
    
    app.start_animation_thread();
    
    app.run_message_loop();
    
    app.shutdown();
}

