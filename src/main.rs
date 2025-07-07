#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread::sleep,
    time::{Duration, Instant},
};
use trayicon::*;
use windows::{
    core::*,
    Win32::Foundation::HWND,
    Win32::UI::Shell::*,
    Win32::UI::WindowsAndMessaging::*,
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
            .map(|i| Icon::from_buffer(i, None, None).unwrap())
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
    ShowMenu,
}

fn safe_message_box(message: &str, title: &str, flags: u32) -> std::result::Result<(), Box<dyn std::error::Error>> {
    unsafe {
        let result = MessageBoxW(
            HWND::default(),
            &HSTRING::from(message),
            &HSTRING::from(title),
            windows::Win32::UI::WindowsAndMessaging::MESSAGEBOX_STYLE(flags),
        );
        if result.0 == 0 {
            return Err("MessageBoxW failed".into());
        }
    }
    Ok(())
}

fn safe_shell_execute(file: &str) -> std::result::Result<(), Box<dyn std::error::Error>> {
    unsafe {
        let ret = ShellExecuteW(
            HWND::default(),
            None,
            &HSTRING::from(file),
            None,
            None,
            SW_SHOWNORMAL,
        );
        if ret.0 as usize <= 32 {
            return Err(format!("ShellExecute failed with code: {}", ret.0 as usize).into());
        }
    }
    Ok(())
}

fn safe_message_loop() -> std::result::Result<(), Box<dyn std::error::Error>> {
    unsafe {
        let mut msg = std::mem::zeroed();
        let bret = GetMessageA(&mut msg, None, 0, 0);
        if bret.as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageA(&msg);
            Ok(())
        } else {
            Err("GetMessageA returned 0 (WM_QUIT)".into())
        }
    }
}


fn main() {
    let (s, r) = std::sync::mpsc::channel::<Events>();
    let icon_id = get_icon_id();
    let icons = (0..4)
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
        if let Err(err) = safe_message_box(&msg, "RustCat Error", MB_OK.0) {
            eprintln!("Failed to show panic dialog: {}", err);
        }
    }));

    let tray_icon = TrayIconBuilder::new()
        .sender(move |e: &Events| {
            let _ = s.send(*e);
        })
        .icon(icons[icon_id][0].clone())
        .tooltip("Nyan~")
        .menu(build_menu(icon_id))
        .on_double_click(Events::RunTaskmgr)
        .on_right_click(Events::ShowMenu)
        .build()
        .unwrap();

    let icon_id = Arc::new(AtomicUsize::new(icon_id));
    let exit = Arc::new(AtomicBool::new(false));
    let tray_icon = Arc::new(Mutex::new(tray_icon));
    
    // Store thread handles for graceful shutdown
    let mut thread_handles = Vec::new();
    
    {
        let exit = exit.clone();
        let icon_id = icon_id.clone();
        let tray_icon = tray_icon.clone();
        let event_thread = std::thread::spawn(move || {
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
                        // Post quit message to main thread's message loop
                        unsafe {
                            PostQuitMessage(0);
                        }
                        break;
                    }
                    Events::RunTaskmgr => {
                        if let Err(err) = safe_shell_execute("taskmgr.exe") {
                            if let Err(msg_err) = safe_message_box(&err.to_string(), "RustCat Error", MB_OK.0) {
                                eprintln!("Failed to show error dialog: {}", msg_err);
                            }
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
                    Events::ShowAboutDialog => {
                        let version = env!("CARGO_PKG_VERSION");
                        let git_hash = option_env!("GIT_HASH").unwrap_or("N/A");
                        let project_page = "https://github.com/bearice/RustCat";
                        let message = format!(
                            "RustCat version {} (Git: {})\nProject Page: {}",
                            version, git_hash, project_page
                        );
                        if let Err(err) = safe_message_box(&message, "About RustCat", (MB_OK | MB_ICONINFORMATION).0) {
                            eprintln!("Failed to show about dialog: {}", err);
                        }
                    },
                    Events::ShowMenu => {
                        tray_icon.lock().unwrap().show_menu().unwrap();
                    },
                }
            }
        });
        thread_handles.push(event_thread);
    }

    {
        let exit = exit.clone();
        let cpu_thread = std::thread::spawn(move || {
            let sleep_interval = 10;
            let mut t1 = match cpu_usage::get_cpu_totals() {
                Ok(totals) => totals,
                Err(e) => {
                    eprintln!("Failed to get initial CPU totals: {}", e);
                    return;
                }
            };
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
                        .map_err(|e| eprintln!("set_icon error: {:?}", e))
                        .unwrap_or(());
                    //set_icon call may fail if pc goes into sleep mode, just ignore them
                }
                animate_counter += sleep_interval;
                if update_counter == 1000 {
                    update_counter = 0;
                    let t2 = match cpu_usage::get_cpu_totals() {
                        Ok(totals) => totals,
                        Err(e) => {
                            eprintln!("Failed to get CPU totals: {}", e);
                            continue;
                        }
                    };
                    let usage = 100.0 - (t2.1 - t1.1) / (t2.0 - t1.0) * 100.0;
                    t1 = t2;
                    speed = (200.0 / (usage / 5.0).clamp(1.0, 20.0)).round() as u64;
                    println!("CPU Usage: {:.2}% speed: {}", usage, speed);
                    tray_icon
                        .lock()
                        .unwrap()
                        .set_tooltip(&format!("CPU Usage: {:.2}%", usage))
                        .map_err(|e| eprintln!("set_tooltip error: {:?}", e))
                        .unwrap_or(());
                }
                update_counter += sleep_interval;
            }
        });
        thread_handles.push(cpu_thread);
    }
    // Main message loop
    while !exit.load(std::sync::atomic::Ordering::Relaxed) {
        match safe_message_loop() {
            Ok(()) => continue,
            Err(err) => {
                if err.to_string().contains("WM_QUIT") {
                    break;
                } else {
                    eprintln!("Message loop error: {}", err);
                    break;
                }
            }
        }
    }
    
    // Signal all threads to exit
    exit.store(true, std::sync::atomic::Ordering::Relaxed);
    
    // Wait for all threads to finish gracefully with timeout
    let shutdown_timeout = Duration::from_secs(5);
    let shutdown_start = Instant::now();
    
    for (i, handle) in thread_handles.into_iter().enumerate() {
        let remaining_time = shutdown_timeout.saturating_sub(shutdown_start.elapsed());
        
        if remaining_time.is_zero() {
            eprintln!("Shutdown timeout reached, some threads may not have finished cleanly");
            break;
        }
        
        // For simplicity, we'll just join immediately since Rust's thread::join doesn't support timeout
        // In a more complex implementation, we could use a separate monitoring thread
        match handle.join() {
            Ok(()) => {
                // Thread finished successfully
            }
            Err(e) => {
                eprintln!("Thread {} panicked during shutdown: {:?}", i, e);
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
            } else if let Err(e) = run_key.delete_value(VALUE_NAME) {
                eprintln!("Failed to delete registry value '{}' (this may be okay if it didn't exist): {}", VALUE_NAME, e);
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
