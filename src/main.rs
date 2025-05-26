#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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
use winapi::um::{
    shellapi::{self},
    winuser::{self},
};
use winreg::RegKey;

mod cpu_usage;
#[allow(dead_code)]
mod icons {
    include!(concat!(env!("OUT_DIR"), "/icons.rs"));
    use trayicon::*;

    pub fn load_icons(id: usize) -> Vec<Icon> {
        [DARK_CAT, LIGHT_CAT, DARK_PARROT, LIGHT_PARROT][id]
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
    RunTaskmgr,
    ToggleRunOnStart,
    ShowAboutDialog,
}

pub fn wchar(string: &str) -> Vec<u16> {
    format!("{}\0", string).encode_utf16().collect::<Vec<_>>()
}

fn main() {
    let (s, r) = std::sync::mpsc::channel::<Events>();
    let icon_id = get_icon_id();
    let icons = (0..4)
        .into_iter()
        .map(icons::load_icons)
        .collect::<Vec<_>>();
    fn build_menu(icon_id: usize) -> MenuBuilder<Events> {
        let run_on_start_enabled = is_run_on_start_enabled(); // Call internally
        MenuBuilder::new()
            .submenu(
                "&Theme",
                MenuBuilder::new()
                    .checkable("&Dark", is_dark(icon_id), Events::ThemeDark)
                    .checkable("&Light", !is_dark(icon_id), Events::ThemeLight),
            )
            .submenu(
                "&Icon",
                MenuBuilder::new()
                    .checkable("&Cat", is_cat(icon_id), Events::IconCat)
                    .checkable("&Parrot", !is_cat(icon_id), Events::IconParrot),
            )
            .separator()
            .checkable(
                "&Run on Start",
                run_on_start_enabled,
                Events::ToggleRunOnStart,
            )
            .separator()
            .item("&About", Events::ShowAboutDialog)
            .separator()
            .item("E&xit", Events::Exit)
    }

    std::panic::set_hook(Box::new(|e| {
        let msg = format!("Panic: {}", e);
        unsafe {
            winuser::MessageBoxW(
                winuser::HWND_DESKTOP,
                msg.as_ptr() as _,
                wchar("RustCat Error").as_ptr() as _,
                winuser::MB_OK,
            );
        }
    }));

    let tray_icon = TrayIconBuilder::new()
        .sender(move |e: &Events| {
            let _ = s.send(e.clone());
        })
        .icon(icons[icon_id][0].clone())
        .tooltip("Nyan~")
        .menu(build_menu(icon_id))
        .on_double_click(Events::RunTaskmgr)
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
                    Events::RunTaskmgr => unsafe {
                        let ret = shellapi::ShellExecuteW(
                            winuser::HWND_DESKTOP,
                            std::ptr::null(),
                            wchar("taskmgr.exe").as_ptr() as _,
                            std::ptr::null(),
                            std::ptr::null(),
                            winuser::SW_SHOWNORMAL,
                        ) as usize;
                        if ret < 32 {
                            let msg = format!("ShellExecute failed: {}", ret);
                            winuser::MessageBoxW(
                                winuser::HWND_DESKTOP,
                                wchar(&msg).as_ptr() as _,
                                wchar("RustCat Error").as_ptr() as _,
                                winuser::MB_OK,
                            );
                        }
                    },
                    Events::ThemeDark => update_icon(icon_id.load(Ordering::Relaxed) & 2),
                    Events::ThemeLight => update_icon(icon_id.load(Ordering::Relaxed) | 1),
                    Events::IconCat => update_icon(icon_id.load(Ordering::Relaxed) & 1),
                    Events::IconParrot => update_icon(icon_id.load(Ordering::Relaxed) | 2),
                    Events::ToggleRunOnStart => {
                        let current_state = is_run_on_start_enabled();
                        set_run_on_start(!current_state);
                        // new_run_on_start_state variable removed
                        tray_icon
                            .lock()
                            .unwrap()
                            .set_menu(&build_menu(icon_id.load(Ordering::Relaxed)))
                            .expect("set_menu for ToggleRunOnStart");
                    }
                    Events::ShowAboutDialog => unsafe {
                        let version = env!("CARGO_PKG_VERSION");
                        let git_hash = option_env!("GIT_HASH").unwrap_or("N/A");
                        let project_page = "https://github.com/bearice/RustCat"; // Hardcoded as per plan
                        let message = format!(
                            "RustCat version {} (Git: {})\nProject Page: {}",
                            version, git_hash, project_page
                        );
                        winuser::MessageBoxW(
                            winuser::HWND_DESKTOP,
                            wchar(&message).as_ptr(),
                            wchar("About RustCat").as_ptr(),
                            winuser::MB_OK | winuser::MB_ICONINFORMATION,
                        );
                    },
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
                        .map_err(|e| println!("set_icon error: {:?}", e))
                        .unwrap_or(());
                    //set_icon call may fail if pc goes into sleep mode, just ignore them
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
                        .map_err(|e| println!("set_tooltip error: {:?}", e))
                        .unwrap_or(());
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

fn is_run_on_start_enabled() -> bool {
    use winreg::enums::*;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(run_key) = hkcu.open_subkey_with_flags(
        "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
        KEY_READ,
    ) {
        // Attempt to get the value. The type of the value doesn't matter as much as its existence.
        // We expect it to be a String (REG_SZ) if it exists.
        if run_key.get_value::<String, _>("RustCat").is_ok() {
            // Optionally, you could check if the value (path) is not empty,
            // but for simplicity, existence is enough.
            return true;
        }
    }
    false
}

fn set_run_on_start(enable: bool) {
    use std::env;
    use winreg::enums::*;
    use winreg::RegKey;

    const RUN_KEY_PATH: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
    const VALUE_NAME: &str = "RustCat";

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    match hkcu.open_subkey_with_flags(RUN_KEY_PATH, KEY_WRITE | KEY_READ) {
        Ok(run_key) => {
            if enable {
                match env::current_exe() {
                    Ok(exe_path) => {
                        let exe_path_str = exe_path.to_string_lossy().to_string();
                        if let Err(e) = run_key.set_value(VALUE_NAME, &exe_path_str) {
                            eprintln!("Failed to set registry value '{}': {}", VALUE_NAME, e);
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to get current executable path: {}", e);
                    }
                }
            } else {
                if let Err(_e) = run_key.delete_value(VALUE_NAME) {
                    // It's okay if the value doesn't exist when trying to delete.
                    // You might want to log this for debugging if it's unexpected.
                    // eprintln!("Failed to delete registry value '{}' (this may be okay if it didn't exist): {}", VALUE_NAME, e);
                }
            }
        }
        Err(e) => {
            eprintln!(
                "Failed to open or create registry subkey '{}': {}",
                RUN_KEY_PATH, e
            );
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
