#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use windows::Win32::UI::WindowsAndMessaging::MB_OK;

mod cpu_usage;
mod events;
mod settings;
mod windows_api;
mod app;

#[allow(dead_code)]
mod icons {
    include!(concat!(env!("OUT_DIR"), "/icons.rs"));
    use trayicon::*;

    pub fn load_icons(id: usize) -> Vec<Icon> {
        [DARK_CAT, LIGHT_CAT, DARK_PARROT, LIGHT_PARROT][id]
            .iter()
            .map(|i| Icon::from_buffer(i, None, None).unwrap())
            .collect()
    }
}

use crate::{
    app::App,
    settings::get_icon_id,
    windows_api::safe_message_box,
};



fn main() {
    let icon_id = get_icon_id();
    let icons = (0..4)
        .map(icons::load_icons)
        .collect::<Vec<_>>();

    std::panic::set_hook(Box::new(|e| {
        let msg = format!("Panic: {}", e);
        if let Err(err) = safe_message_box(&msg, "RustCat Error", MB_OK.0) {
            eprintln!("Failed to show panic dialog: {}", err);
        }
    }));

    let app = App::new(&icons, icon_id).expect("Failed to create app");
    
    app.start_animation_thread(&icons);
    
    app.run_message_loop();
    
    app.shutdown();
}

