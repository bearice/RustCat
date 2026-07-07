use crate::icon_manager::Theme;
use crate::platform::SettingsManager;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub struct LinuxSettingsManager;

impl SettingsManager for LinuxSettingsManager {
    fn get_current_icon() -> String {
        read_setting("IconName").unwrap_or_else(|| "cat".to_string())
    }

    fn set_current_icon(icon_name: &str) {
        write_setting("IconName", icon_name);
    }

    fn get_current_theme() -> Theme {
        if let Some(theme_str) = read_setting("Theme") {
            match theme_str.as_str() {
                "dark" => Theme::Dark,
                "light" => Theme::Light,
                _ => Theme::from_system(),
            }
        } else {
            Theme::from_system()
        }
    }

    fn set_current_theme(theme: Option<Theme>) {
        match theme {
            Some(theme) => write_setting("Theme", &theme.to_string()),
            None => remove_setting("Theme"),
        }
    }

    fn is_run_on_start_enabled() -> bool {
        autostart_desktop_path().exists()
    }

    fn set_run_on_start(enable: bool) {
        let desktop_path = autostart_desktop_path();

        if enable {
            if let Ok(exe_path) = std::env::current_exe() {
                let exe_str = exe_path.to_string_lossy().to_string();
                // Escape the Exec value per the freedesktop Desktop Entry spec so
                // paths containing spaces / other reserved metacharacters still
                // launch the binary at login.
                let exec_value = escape_exec_value(&exe_str);
                let desktop_content = format!(
                    "[Desktop Entry]\n\
Type=Application\n\
Name=RustCat\n\
Comment=CPU usage monitor tray cat\n\
Exec={exec_value}\n\
Icon=rustcat\n\
Terminal=false\n\
X-KDE-autostart-phase=2\n\
NoDisplay=false\n"
                );
                if let Some(parent) = desktop_path.parent() {
                    if let Err(e) = fs::create_dir_all(parent) {
                        eprintln!("Failed to create autostart directory: {}", e);
                        return;
                    }
                }
                if let Err(e) = fs::write(&desktop_path, desktop_content) {
                    eprintln!("Failed to write autostart desktop file: {}", e);
                }
            }
        } else if let Err(e) = fs::remove_file(&desktop_path) {
            if e.kind() != std::io::ErrorKind::NotFound {
                eprintln!("Failed to remove autostart desktop file: {}", e);
            }
        }
    }

    fn is_dark_mode_enabled() -> bool {
        // Prefer KDE's kreadconfig (works on Plasma 5/6)
        for tool in ["kreadconfig6", "kreadconfig5"] {
            let output = Command::new(tool)
                .args(["--group", "General", "--key", "ColorScheme"])
                .output();
            if let Ok(out) = output {
                if out.status.success() {
                    let scheme = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    // KDE color scheme names containing "Dark" are dark themes
                    // (e.g. "Breeze Dark", "Breeze-Dark")
                    if scheme.to_lowercase().contains("dark") {
                        return true;
                    }
                    if scheme.to_lowercase().contains("light") {
                        return false;
                    }
                }
            }
        }

        // Fallback: parse ~/.config/kdeglobals directly
        if let Some(home) = dirs::config_dir() {
            let kdeglobals = home.join("kdeglobals");
            if let Ok(content) = fs::read_to_string(&kdeglobals) {
                let mut in_general = false;
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with('[') {
                        in_general = trimmed == "[General]";
                        continue;
                    }
                    if in_general {
                        if let Some((key, value)) = trimmed.split_once('=') {
                            if key.trim() == "ColorScheme" {
                                return value.trim().to_lowercase().contains("dark");
                            }
                        }
                    }
                }
            }
        }

        false
    }

    fn migrate_legacy_settings() {
        // No legacy settings to migrate on Linux
    }
}

fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("rustcat")
}

/// Escape a string for use as the value of a freedesktop desktop entry `Exec`
/// key. Reserved characters must be backslash-escaped, otherwise a path with
/// spaces (e.g. `/home/me/Rust Cat/rust_cat`) is split into tokens at login.
fn escape_exec_value(s: &str) -> String {
    const RESERVED: &[char] = &[
        ' ', '\t', '"', '\'', '\\', '`', '$', '*', '?', '#', '(', ')', '>', '<',
        '~', '|', '&', ';',
    ];
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if RESERVED.contains(&ch) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

fn settings_path() -> PathBuf {
    config_dir().join("settings.conf")
}

fn autostart_desktop_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("autostart")
        .join("rustcat.desktop")
}

fn read_setting(key: &str) -> Option<String> {
    let content = fs::read_to_string(settings_path()).ok()?;
    for line in content.lines() {
        if let Some((k, v)) = line.split_once('=') {
            if k.trim() == key {
                return Some(v.trim().to_string());
            }
        }
    }
    None
}

fn write_setting(key: &str, value: &str) {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            eprintln!("Failed to create config directory: {}", e);
            return;
        }
    }

    let mut content = fs::read_to_string(&path).unwrap_or_default();
    let mut found = false;
    let mut new_lines = Vec::new();
    for line in content.lines() {
        if let Some((k, _)) = line.split_once('=') {
            if k.trim() == key {
                new_lines.push(format!("{}={}", key, value));
                found = true;
                continue;
            }
        }
        new_lines.push(line.to_string());
    }
    if !found {
        new_lines.push(format!("{}={}", key, value));
    }

    content = new_lines.join("\n");
    if !content.ends_with('\n') {
        content.push('\n');
    }

    if let Err(e) = fs::write(&path, content) {
        eprintln!("Failed to write setting '{}': {}", key, e);
    }
}

fn remove_setting(key: &str) {
    let path = settings_path();
    let Ok(content) = fs::read_to_string(&path) else {
        return;
    };
    let new_lines: Vec<String> = content
        .lines()
        .filter(|line| {
            line.split_once('=').map(|(k, _)| k.trim() != key).unwrap_or(true)
        })
        .map(String::from)
        .collect();
    if let Err(e) = fs::write(&path, format!("{}\n", new_lines.join("\n"))) {
        eprintln!("Failed to remove setting '{}': {}", key, e);
    }
}