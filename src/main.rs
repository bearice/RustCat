#![cfg_attr(release, windows_subsystem = "windows")]

use core::mem::MaybeUninit;
use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread::sleep,
    time::Duration,
};
use trayicon::*;
use winapi::um::winuser::{self};
use winreg::RegKey;

mod cpu_usage;
#[allow(dead_code)]
mod icons {
    include!(concat!(env!("OUT_DIR"), "/icons.rs"));
    use trayicon::*;

    pub fn load_icons(id: u32) -> Vec<Icon> {
        [DARK_CAT, LIGHT_CAT, DARK_PARROT, LIGHT_PARROT][id as usize]
            .iter()
            .map(|i| Icon::from_buffer(*i, None, None).unwrap())
            .collect()
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum Events {
    Exit,
    ThemeDark,
    ThemeLight,
    IconCat,
    IconParrot,
}

fn main() {
    let (s, r) = std::sync::mpsc::channel::<Events>();
    let icon_id = get_icon_id();
    let icons = (0..4)
        .into_iter()
        .map(icons::load_icons)
        .collect::<Vec<_>>();
    fn build_menu(icon_id: usize) -> MenuBuilder<Events> {
        MenuBuilder::new()
            .submenu(
                "Theme",
                MenuBuilder::new()
                    .checkable("D&ark", is_dark(icon_id), Events::ThemeDark)
                    .checkable("L&ight", !is_dark(icon_id), Events::ThemeLight),
            )
            .submenu(
                "Icon",
                MenuBuilder::new()
                    .checkable("C&at", is_cat(icon_id), Events::IconCat)
                    .checkable("P&arrot", !is_cat(icon_id), Events::IconParrot),
            )
            .separator()
            .item("E&xit", Events::Exit)
    }
    let tray_icon = TrayIconBuilder::new()
        .sender(s)
        .icon(icons[icon_id as usize][0].clone())
        .tooltip("Nyan~")
        .menu(build_menu(icon_id))
        .build()
        .unwrap();

    let icon_id = Arc::new(AtomicUsize::new(icon_id));
    let exit = Arc::new(AtomicBool::new(false));
    let tray_icon = Arc::new(Mutex::new(tray_icon));
    {
        let exit = exit.clone();
        let icon_id = icon_id.clone();
        let tray_icon = tray_icon.clone();
        std::thread::spawn(move || {
            let update_icon = |id| {
                set_icon_id(id);
                icon_id.store(id, Ordering::Relaxed);
                tray_icon
                    .lock()
                    .unwrap()
                    .set_menu(&build_menu(id))
                    .expect("set_menu")
            };
            for m in r.iter() {
                match m {
                    Events::Exit => {
                        exit.store(true, Ordering::Relaxed);
                        break;
                    }
                    Events::ThemeDark => update_icon(icon_id.load(Ordering::Relaxed) & 2),
                    Events::ThemeLight => update_icon(icon_id.load(Ordering::Relaxed) | 1),
                    Events::IconCat => update_icon(icon_id.load(Ordering::Relaxed) & 1),
                    Events::IconParrot => update_icon(icon_id.load(Ordering::Relaxed) | 2),
                }
            }
        });
    }

    {
        let exit = exit.clone();
        std::thread::spawn(move || {
            let sleep_interval = 10;
            let mut t1 = cpu_usage::get_cpu_totals().unwrap();
            let mut update_counter = 0;
            let mut animate_counter = 0;
            let mut icon_index = 0;
            let mut speed = 200;
            while !exit.load(Ordering::Relaxed) {
                sleep(Duration::from_millis(sleep_interval));
                let icons = &icons[icon_id.load(Ordering::Relaxed)];
                if animate_counter >= speed {
                    animate_counter = 0;
                    icon_index += 1;
                    icon_index %= icons.len();
                    tray_icon
                        .lock()
                        .unwrap()
                        .set_icon(&icons[icon_index])
                        .expect("set_icon");
                }
                animate_counter += sleep_interval;
                if update_counter == 1000 {
                    update_counter = 0;
                    let t2 = cpu_usage::get_cpu_totals().unwrap();
                    let usage = 100.0 - (t2.1 - t1.1) / (t2.0 - t1.0) * 100.0;
                    t1 = t2;
                    speed = (200.0 / f64::max(1.0, f64::min(20.0, usage / 5.0))).round() as u64;
                    println!("CPU Usage: {:.2}% speed: {}", usage, speed);
                    tray_icon
                        .lock()
                        .unwrap()
                        .set_tooltip(&format!("CPU Usage: {:.2}%", usage))
                        .expect("set_tooltip");
                }
                update_counter += sleep_interval;
            }
        });
    }
    // Your applications message loop. Because all applications require an
    // application loop, you are best served using an `winit` crate.
    while !exit.load(std::sync::atomic::Ordering::Relaxed) {
        unsafe {
            let mut msg = MaybeUninit::uninit();
            let bret = winuser::GetMessageA(msg.as_mut_ptr(), 0 as _, 0, 0);
            if bret > 0 {
                winuser::TranslateMessage(msg.as_ptr());
                winuser::DispatchMessageA(msg.as_ptr());
            } else {
                break;
            }
        }
    }
}

fn is_dark(id: usize) -> bool {
    id & 1 == 0
}

fn is_cat(id: usize) -> bool {
    id & 2 == 0
}

fn get_icon_id() -> usize {
    let key = RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    if let Ok(sub_key) = key.open_subkey_with_flags("Software\\RustCat", winreg::enums::KEY_READ) {
        if let Ok(value) = sub_key.get_value::<u32, &str>("IconId") {
            return value as usize;
        }
    }
    // return default value based on system theme
    if is_dark_mode_enabled() {
        0
    } else {
        1
    }
}

fn set_icon_id(id: usize) {
    let key = RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    let sub_key = if let Ok(sub_key) = key.open_subkey_with_flags(
        "Software\\RustCat",
        winreg::enums::KEY_WRITE | winreg::enums::KEY_READ,
    ) {
        sub_key
    } else {
        key.create_subkey_with_flags(
            "Software\\RustCat",
            winreg::enums::KEY_WRITE | winreg::enums::KEY_READ,
        )
        .expect("create_subkey_with_flags")
        .0
    };
    sub_key
        .set_value("IconId", &(id as u32))
        .expect("set_value");
}

fn is_dark_mode_enabled() -> bool {
    let hkcu = RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    if let Ok(subkey) =
        hkcu.open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize")
    {
        if let Ok(dword) = subkey.get_value::<u32, _>("AppsUseLightTheme") {
            dword == 0
        } else {
            false
        }
    } else {
        false
    }
}
