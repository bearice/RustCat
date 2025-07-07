use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread::sleep,
    time::Duration,
};
use trayicon::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::{
    cpu_usage,
    events::{Events, build_menu},
    settings::{set_icon_id, is_run_on_start_enabled, set_run_on_start},
    windows_api::{safe_message_box, safe_shell_execute, safe_message_loop},
};

pub struct App {
    pub tray_icon: Arc<Mutex<TrayIcon<Events>>>,
    pub icon_id: Arc<AtomicUsize>,
    pub exit: Arc<AtomicBool>,
    pub icons: Arc<Vec<Vec<Icon>>>,
}

impl App {
    pub fn new(icons: Vec<Vec<Icon>>, initial_icon_id: usize) -> Result<Self, Box<dyn std::error::Error>> {
        let (s, r) = std::sync::mpsc::channel::<Events>();
        
        let tray_icon = TrayIconBuilder::new()
            .sender(move |e: &Events| {
                let _ = s.send(*e);
            })
            .icon(icons[initial_icon_id][0].clone())
            .tooltip("Nyan~")
            .menu(build_menu(initial_icon_id))
            .on_double_click(Events::RunTaskmgr)
            .on_right_click(Events::ShowMenu)
            .build()?;

        let icon_id = Arc::new(AtomicUsize::new(initial_icon_id));
        let exit = Arc::new(AtomicBool::new(false));
        let tray_icon = Arc::new(Mutex::new(tray_icon));
        let icons = Arc::new(icons);
        let app = App {
            tray_icon,
            icon_id,
            exit,
            icons,
        };

        app.start_event_thread(r);
        
        Ok(app)
    }

    pub fn start_event_thread(&self, receiver: std::sync::mpsc::Receiver<Events>) {
        let exit = self.exit.clone();
        let icon_id = self.icon_id.clone();
        let tray_icon = self.tray_icon.clone();
        
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
            
            for event in receiver.iter() {
                match event {
                    Events::Exit => {
                        exit.store(true, Ordering::Relaxed);
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
    }

    pub fn start_animation_thread(&self) {
        let exit = self.exit.clone();
        let icon_id = self.icon_id.clone();
        let tray_icon = self.tray_icon.clone();
        let icons = self.icons.clone();
                
        std::thread::spawn(move || {
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
    }

    pub fn run_message_loop(&self) {
        while !self.exit.load(Ordering::Relaxed) {
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
    }

    pub fn shutdown(&self) {
        self.exit.store(true, Ordering::Relaxed);
    }
}