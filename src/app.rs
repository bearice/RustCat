use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::events::{build_menu, Events};
use crate::icon_manager::{IconManager, Theme};
use crate::platform::{CpuMonitor, GpuMonitor, SettingsManager, SystemIntegration};
use crate::platform::{
    CpuMonitorImpl, GpuMonitorImpl, SettingsManagerImpl, SystemIntegrationImpl,
};
use crate::debug;

use trayicon::*;

/// The usage source that drives the animation speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationSource {
    Cpu,
    Gpu,
    Both,
}

impl AnimationSource {
    /// Parse from the persisted string form; unknown values default to CPU.
    pub fn from_str(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "gpu" => AnimationSource::Gpu,
            "both" => AnimationSource::Both,
            _ => AnimationSource::Cpu,
        }
    }

    /// Persisted string form (registry value / defaults key / settings.conf).
    pub fn as_str(self) -> &'static str {
        match self {
            AnimationSource::Cpu => "cpu",
            AnimationSource::Gpu => "gpu",
            AnimationSource::Both => "both",
        }
    }

    /// Human-readable label for menus and logs.
    pub fn label(self) -> &'static str {
        match self {
            AnimationSource::Cpu => "CPU",
            AnimationSource::Gpu => "GPU",
            AnimationSource::Both => "CPU + GPU",
        }
    }
}

/// Sample CPU and/or GPU usage depending on the selected source.
///
/// A `None` component means it was not sampled (the source does not need it)
/// or the read failed.
fn sample_usage(source: AnimationSource) -> (Option<f64>, Option<f64>) {
    let cpu = match source {
        AnimationSource::Gpu => None,
        _ => CpuMonitorImpl::get_cpu_usage()
            .map_err(|e| eprintln!("Failed to get CPU usage: {}", e))
            .ok(),
    };
    let gpu = match source {
        AnimationSource::Cpu => None,
        _ => GpuMonitorImpl::get_gpu_usage()
            .map_err(|e| eprintln!("Failed to get GPU usage: {}", e))
            .ok(),
    };
    (cpu, gpu)
}

/// The usage value that drives the animation: CPU, GPU, or the max of both.
/// `None` means no usable data was available this sample.
fn effective_usage(
    source: AnimationSource,
    cpu: Option<f64>,
    gpu: Option<f64>,
) -> Option<f64> {
    match source {
        AnimationSource::Cpu => cpu,
        AnimationSource::Gpu => gpu,
        AnimationSource::Both => match (cpu, gpu) {
            (Some(c), Some(g)) => Some(c.max(g)),
            (c, g) => c.or(g),
        },
    }
}

/// Format a usage value for tooltips; `None` renders as "n/a".
fn fmt_pct(usage: Option<f64>) -> String {
    match usage {
        Some(v) => format!("{:.2}%", v),
        None => "n/a".to_string(),
    }
}

fn is_sleep_time() -> bool {
    let hour = SystemIntegrationImpl::get_local_hour();
    // Check if current hour is between 22:00 and 6:00
    !(6..22).contains(&hour)
}

// On macos, ui updates must be done on the main thread.
// This is a workaround to ensure that UI updates are dispatched correctly.
#[cfg(target_os = "macos")]
use dispatch;
#[cfg(target_os = "macos")]
fn ui_update<F: FnOnce() + Send + 'static>(f: F) {
    dispatch::Queue::main().exec_async(f);
}
// Windows and Linux (D-Bus StatusNotifierItem) do not require UI updates
// to be dispatched on a specific thread — the trayicon backend handles
// thread-safety internally.
#[cfg(not(target_os = "macos"))]
fn ui_update<F: FnOnce() + Send + 'static>(f: F) {
    f();
}
pub struct App {
    tray_icon: Arc<Mutex<TrayIcon<Events>>>,
    icon_manager: Arc<IconManager>,
    pub(crate) exit_flag: Arc<AtomicBool>,
    event_receiver: Option<mpsc::Receiver<Events>>,
    icon_name: Arc<Mutex<String>>,
    theme: Arc<Mutex<Theme>>,
    animation_source: Arc<Mutex<AnimationSource>>,
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
        let animation_source = SettingsManagerImpl::get_animation_source();
        let initial_icons = icon_manager
            .get_icon_set(initial_icon, Some(theme))
            .ok_or("Invalid initial icon name")?;

        let tray_icon = TrayIconBuilder::new()
            .sender(move |e: &Events| {
                let _ = sender.send(e.clone());
            })
            .icon(initial_icons[0].clone())
            .tooltip("~Nyan~ RustCat - CPU/GPU Usage Monitor")
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
            animation_source: Arc::new(Mutex::new(animation_source)),
        })
    }

    pub fn start_animation_thread(&self) {
        let exit_flag = self.exit_flag.clone();
        let tray_icon = self.tray_icon.clone();
        let icon_manager = self.icon_manager.clone();
        let icon_name = self.icon_name.clone();
        let theme = self.theme.clone();
        let animation_source = self.animation_source.clone();

        thread::spawn(move || {
            let sleep_interval = 10;
            let mut update_counter = 0;
            let mut animate_counter = 0;
            let mut icon_index = 0;
            let mut speed = 200;
            let mut idle_counter = 0;
            let idle_threshold = 60 * 1000; // 1 minute in milliseconds
            let mut is_sleeping = false;

            while !exit_flag.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(sleep_interval));

                let current_icon_name = icon_name.lock().unwrap().clone();
                let current_theme = *theme.lock().unwrap();
                
                // Determine which icon set to use based on idle state
                let icon_set_name = if is_sleeping && current_icon_name == "cat" {
                    "sleep"
                } else {
                    &current_icon_name
                };
                
                let icons = match icon_manager.get_icon_set(icon_set_name, Some(current_theme))
                {
                    Some(icons) => icons,
                    None => {
                        eprintln!("Invalid icon name: {}", icon_set_name);
                        continue;
                    }
                };

                if animate_counter >= speed {
                    animate_counter = 0;
                    icon_index += 1;
                    icon_index %= icons.len();
                    {
                        let tray_icon_clone = tray_icon.clone();
                        let icon_data = icons[icon_index].clone();
                        ui_update(move || {
                            if let Ok(mut tray) = tray_icon_clone.lock() {
                                if let Err(e) = tray.set_icon(&icon_data) {
                                    eprintln!("set_icon error: {:?}", e);
                                }
                            }
                        });
                    }
                }
                animate_counter += sleep_interval;

                if update_counter >= 1000 {
                    update_counter = 0;
                    let source = *animation_source.lock().unwrap();
                    let (cpu_usage, gpu_usage) = sample_usage(source);
                    let usage = effective_usage(source, cpu_usage, gpu_usage);

                    if let Some(usage) = usage {
                        speed = (200.0 / (usage / 5.0).clamp(1.0_f64, 20.0_f64)).round() as u64;
                        debug!("{} Usage: {:.2}% speed: {}", source.label(), usage, speed);

                        // Check if the machine is idle (less than 5% usage) and
                        // it's sleep time (22:00-6:00)
                        if usage < 5.0 && is_sleep_time() {
                            idle_counter += 1000; // Add the update interval
                            if idle_counter >= idle_threshold && !is_sleeping {
                                is_sleeping = true;
                                icon_index = 0; // Reset animation to start from first sleeping frame
                                debug!("Usage has been idle for 1 minute during sleep hours, switching to sleeping cat");
                            }
                        } else {
                            idle_counter = 0;
                            if is_sleeping {
                                is_sleeping = false;
                                icon_index = 0; // Reset animation
                                if usage >= 5.0 {
                                    debug!("Activity detected, switching back to normal cat");
                                } else {
                                    debug!("Outside sleep hours, switching back to normal cat");
                                }
                            }
                        }
                    } else {
                        debug!(
                            "No {} usage data available, keeping previous speed",
                            source.label()
                        );
                    }

                    {
                        let tray_icon_clone = tray_icon.clone();
                        let tooltip = if is_sleeping && current_icon_name == "cat" {
                            "Shhhh, Your CPU is sleeping...💤".to_string()
                        } else {
                            match source {
                                AnimationSource::Cpu => {
                                    format!("CPU Usage: {}", fmt_pct(cpu_usage))
                                }
                                AnimationSource::Gpu => {
                                    format!("GPU Usage: {}", fmt_pct(gpu_usage))
                                }
                                AnimationSource::Both => format!(
                                    "CPU: {} | GPU: {}",
                                    fmt_pct(cpu_usage),
                                    fmt_pct(gpu_usage)
                                ),
                            }
                        };
                        ui_update(move || {
                            if let Ok(mut tray) = tray_icon_clone.lock() {
                                if let Err(e) = tray.set_tooltip(&tooltip) {
                                    eprintln!("set_tooltip error: {:?}", e);
                                }
                            }
                        });
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
                    Events::SetAnimationSource(source) => {
                        SettingsManagerImpl::set_animation_source(source);
                        *self.animation_source.lock().unwrap() = source;
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
                            "RustCat version {} (Git: {})\nProject Page: {}",
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
        let tray_icon = self.tray_icon.clone();
        let icon_manager = self.icon_manager.clone();
        ui_update(move || {
            if let Ok(mut tray) = tray_icon.lock() {
                if let Err(e) = tray.set_menu(&build_menu(&icon_manager)) {
                    eprintln!("Failed to update menu: {}", e);
                }
            }
        });
    }

    pub fn shutdown(&self) {
        // Platform-specific shutdown logic can be added here
        debug!("Shutting down RustCat...");
    }
}
