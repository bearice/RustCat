use std::io;

#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(windows)]
pub mod windows;

/// Cross-platform CPU usage monitoring trait
pub trait CpuMonitor {
    /// Returns CPU usage percentage as a float (0.0 to 100.0)
    fn get_cpu_usage() -> io::Result<f64>;
}

/// Cross-platform settings management trait
pub trait SettingsManager {
    fn get_current_icon() -> String;
    fn set_current_icon(icon_name: &str);
    fn get_current_theme() -> crate::icon_manager::Theme;
    fn set_current_theme(theme: Option<crate::icon_manager::Theme>);
    fn is_run_on_start_enabled() -> bool;
    fn set_run_on_start(enable: bool);
    fn is_dark_mode_enabled() -> bool;
    fn migrate_legacy_settings();
}

/// Cross-platform system integration trait
pub trait SystemIntegration {
    fn show_dialog(message: &str, title: &str) -> Result<(), Box<dyn std::error::Error>>;
    fn open_system_monitor() -> Result<(), Box<dyn std::error::Error>>;
}

/// Platform-specific implementation type aliases
#[cfg(windows)]
pub type CpuMonitorImpl = windows::WindowsCpuMonitor;
#[cfg(target_os = "macos")]
pub type CpuMonitorImpl = macos::MacosCpuMonitor;

#[cfg(windows)]
pub type SettingsManagerImpl = windows::WindowsSettingsManager;
#[cfg(target_os = "macos")]
pub type SettingsManagerImpl = macos::MacosSettingsManager;

#[cfg(windows)]
pub type SystemIntegrationImpl = windows::WindowsSystemIntegration;
#[cfg(target_os = "macos")]
pub type SystemIntegrationImpl = macos::MacosSystemIntegration;
