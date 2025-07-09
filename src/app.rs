use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::events::{build_menu, Events};
use crate::icon_manager::{IconManager, Theme};
use crate::platform::{CpuMonitor, SettingsManager, SystemIntegration};
use crate::platform::{CpuMonitorImpl, SettingsManagerImpl, SystemIntegrationImpl};

use trayicon::*;

pub struct App {
    tray_icon: Arc<Mutex<TrayIcon<Events>>>,
    icon_manager: Arc<IconManager>,
    pub(crate) exit_flag: Arc<AtomicBool>,
    event_receiver: Option<mpsc::Receiver<Events>>,
    icon_name: Arc<Mutex<String>>,
    theme: Arc<Mutex<Theme>>,
}

impl App {
    pub fn new(
        icon_manager: IconManager,
        initial_icon: &str,
        initial_theme: Option<Theme>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let (sender, receiver) = mpsc::channel::<Events>();
        let icon_manager = Arc::new(icon_manager);
        let exit_flag = Arc::new(AtomicBool::new(false));

        let theme = initial_theme.unwrap_or_else(SettingsManagerImpl::get_current_theme);
        let initial_icons = icon_manager
            .get_icon_set(initial_icon, Some(theme))
            .ok_or("Invalid initial icon name")?;

        let tray_icon = TrayIconBuilder::new()
            .sender(move |e: &Events| {
                let _ = sender.send(e.clone());
            })
            .icon(initial_icons[0].clone())
            .tooltip("~Nyan~ RustCat - CPU Usage Monitor")
            .menu(build_menu(&icon_manager))
            .on_right_click(Events::ShowMenu)
            .on_double_click(Events::RunTaskmgr)
            .build()?;

        Ok(App {
            tray_icon: Arc::new(Mutex::new(tray_icon)),
            icon_manager,
            exit_flag,
            event_receiver: Some(receiver),
            icon_name: Arc::new(Mutex::new(initial_icon.to_string())),
            theme: Arc::new(Mutex::new(theme)),
        })
    }

    pub fn start_animation_thread(&self) {
        let exit_flag = self.exit_flag.clone();
        let tray_icon = self.tray_icon.clone();
        let icon_manager = self.icon_manager.clone();
        let icon_name = self.icon_name.clone();
        let theme = self.theme.clone();

        thread::spawn(move || {
            let sleep_interval = 10;
            let mut update_counter = 0;
            let mut animate_counter = 0;
            let mut icon_index = 0;
            let mut speed = 200;

            while !exit_flag.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(sleep_interval));

                let current_icon_name = icon_name.lock().unwrap().clone();
                let current_theme = *theme.lock().unwrap();
                let icons = match icon_manager.get_icon_set(&current_icon_name, Some(current_theme))
                {
                    Some(icons) => icons,
                    None => {
                        eprintln!("Invalid icon name: {}", current_icon_name);
                        continue;
                    }
                };

                if animate_counter >= speed {
                    animate_counter = 0;
                    icon_index += 1;
                    icon_index %= icons.len();
                    if let Ok(mut tray) = tray_icon.lock() {
                        if let Err(e) = tray.set_icon(&icons[icon_index]) {
                            eprintln!("set_icon error: {:?}", e);
                        }
                    }
                }
                animate_counter += sleep_interval;

                if update_counter >= 1000 {
                    update_counter = 0;
                    let usage = match CpuMonitorImpl::get_cpu_usage() {
                        Ok(usage) => usage,
                        Err(e) => {
                            eprintln!("Failed to get CPU usage: {}", e);
                            continue;
                        }
                    };
                    speed = (200.0 / (usage / 5.0).clamp(1.0_f64, 20.0_f64)).round() as u64;
                    println!("CPU Usage: {:.2}% speed: {}", usage, speed);

                    if let Ok(mut tray) = tray_icon.lock() {
                        if let Err(e) = tray.set_tooltip(&format!("CPU Usage: {:.2}%", usage)) {
                            eprintln!("set_tooltip error: {:?}", e);
                        }
                    }
                }
                update_counter += sleep_interval;
            }
        });
    }

    pub fn run(mut self) {
        if let Some(receiver) = self.event_receiver.take() {
            for event in receiver {
                match event {
                    Events::Exit => {
                        self.exit_flag.store(true, Ordering::Relaxed);
                        self.shutdown();
                        // For macOS, we need to trigger NSApplication termination
                        #[cfg(target_os = "macos")]
                        {
                            use objc2_app_kit::NSApplication;
                            use objc2_foundation::MainThreadMarker;
                            let app = NSApplication::sharedApplication(unsafe {
                                MainThreadMarker::new_unchecked()
                            });
                            unsafe { app.terminate(None) };
                        }
                        break;
                    }
                    Events::RunTaskmgr => {
                        if let Err(e) = SystemIntegrationImpl::open_system_monitor() {
                            eprintln!("Failed to open system monitor: {}", e);
                        }
                    }
                    Events::SetTheme(theme) => {
                        SettingsManagerImpl::set_current_theme(Some(theme));
                        *self.theme.lock().unwrap() = theme;
                        self.update_menu();
                    }
                    Events::SetIcon(icon_name) => {
                        SettingsManagerImpl::set_current_icon(&icon_name);
                        *self.icon_name.lock().unwrap() = icon_name;
                        self.update_menu();
                    }
                    Events::ToggleRunOnStart => {
                        let current_state = SettingsManagerImpl::is_run_on_start_enabled();
                        SettingsManagerImpl::set_run_on_start(!current_state);
                        self.update_menu();
                    }
                    Events::ShowAboutDialog => {
                        let version = env!("CARGO_PKG_VERSION");
                        let git_hash = option_env!("GIT_HASH").unwrap_or("N/A");
                        let project_page = "https://github.com/bearice/RustCat";
                        let message = format!(
                            "RustCat version {} (Git: {})\\nProject Page: {}",
                            version, git_hash, project_page
                        );

                        if let Err(e) =
                            SystemIntegrationImpl::show_dialog(&message, "About RustCat")
                        {
                            eprintln!("Failed to show about dialog: {}", e);
                        }
                    }
                    Events::ShowMenu => {
                        if let Ok(mut tray) = self.tray_icon.lock() {
                            if let Err(e) = tray.show_menu() {
                                eprintln!("Failed to show menu: {}", e);
                            }
                        }
                    }
                }
            }
        }
    }

    fn update_menu(&self) {
        if let Ok(mut tray) = self.tray_icon.lock() {
            if let Err(e) = tray.set_menu(&build_menu(&self.icon_manager)) {
                eprintln!("Failed to update menu: {}", e);
            }
        }
    }

    pub fn shutdown(&self) {
        // Platform-specific shutdown logic can be added here
        println!("Shutting down RustCat...");
    }
}
