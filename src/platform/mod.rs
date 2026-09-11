use std::io;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(windows)]
pub mod windows;

/// Cross-platform CPU usage monitoring trait
pub trait CpuMonitor {
    /// Returns CPU usage percentage as a float (0.0 to 100.0)
    fn get_cpu_usage() -> io::Result<f64>;
}

/// A GPU device discovered by the platform monitor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuDevice {
    /// Platform-specific device id, stable for the current boot.
    pub id: String,
    /// Human-readable name for the tray menu.
    pub name: String,
}

/// Cross-platform GPU usage monitoring trait
pub trait GpuMonitor {
    /// Enumerate the GPU devices that expose utilization.
    fn enumerate_gpus() -> Vec<GpuDevice>;

    /// Returns GPU usage percentage as a float (0.0 to 100.0) for a scope.
    ///
    /// `scope` = `None` covers all devices (max across devices);
    /// `scope` = `Some(id)` covers only that device (max across its
    /// engines). Returns `Err` when the scope names a device that no
    /// longer exists, so callers can fall back to all devices.
    ///
    /// Returns `Err` when no GPU usage source is available (no GPU, no
    /// driver, no `nvidia-smi`, ...).
    fn get_gpu_usage(scope: Option<&str>) -> io::Result<f64>;
}

/// Cross-platform settings management trait
pub trait SettingsManager {
    fn get_current_icon() -> String;
    fn set_current_icon(icon_name: &str);
    fn get_current_theme() -> crate::icon_manager::Theme;
    fn set_current_theme(theme: Option<crate::icon_manager::Theme>);
    fn get_animation_source() -> crate::app::AnimationSource;
    fn set_animation_source(source: crate::app::AnimationSource);
    /// Selected GPU device id; `None` = all GPUs (the default).
    fn get_gpu_scope() -> Option<String>;
    fn set_gpu_scope(scope: Option<String>);
    fn is_run_on_start_enabled() -> bool;
    fn set_run_on_start(enable: bool);
    fn is_dark_mode_enabled() -> bool;
    fn migrate_legacy_settings();
}

/// Cross-platform system integration trait
pub trait SystemIntegration {
    fn show_dialog(message: &str, title: &str) -> Result<(), Box<dyn std::error::Error>>;
    fn open_system_monitor() -> Result<(), Box<dyn std::error::Error>>;
    fn get_local_hour() -> u32;
}

/// Platform-specific implementation type aliases
#[cfg(windows)]
pub type CpuMonitorImpl = windows::WindowsCpuMonitor;
#[cfg(target_os = "macos")]
pub type CpuMonitorImpl = macos::MacosCpuMonitor;
#[cfg(target_os = "linux")]
pub type CpuMonitorImpl = linux::LinuxCpuMonitor;

#[cfg(windows)]
pub type GpuMonitorImpl = windows::WindowsGpuMonitor;
#[cfg(target_os = "macos")]
pub type GpuMonitorImpl = macos::MacosGpuMonitor;
#[cfg(target_os = "linux")]
pub type GpuMonitorImpl = linux::LinuxGpuMonitor;

#[cfg(windows)]
pub type SettingsManagerImpl = windows::WindowsSettingsManager;
#[cfg(target_os = "macos")]
pub type SettingsManagerImpl = macos::MacosSettingsManager;
#[cfg(target_os = "linux")]
pub type SettingsManagerImpl = linux::LinuxSettingsManager;

#[cfg(windows)]
pub type SystemIntegrationImpl = windows::WindowsSystemIntegration;
#[cfg(target_os = "macos")]
pub type SystemIntegrationImpl = macos::MacosSystemIntegration;
#[cfg(target_os = "linux")]
pub type SystemIntegrationImpl = linux::LinuxSystemIntegration;
