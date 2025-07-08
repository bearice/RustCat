use crate::icon_manager::Theme;
use crate::platform::SettingsManager;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub struct MacosSettingsManager;

impl SettingsManager for MacosSettingsManager {
    fn get_current_icon() -> String {
        get_preference("IconName").unwrap_or_else(|| "cat".to_string())
    }

    fn set_current_icon(icon_name: &str) {
        set_preference("IconName", icon_name);
    }

    fn get_current_theme() -> Theme {
        if let Some(theme_str) = get_preference("Theme") {
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
            Some(theme) => set_preference("Theme", &theme.to_string()),
            None => remove_preference("Theme"),
        }
    }

    fn is_run_on_start_enabled() -> bool {
        let plist_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("Library/LaunchAgents/com.bearice.rustcat.plist");
        plist_path.exists()
    }

    fn set_run_on_start(enable: bool) {
        let launch_agents_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("Library/LaunchAgents");

        if !launch_agents_dir.exists() {
            if let Err(e) = fs::create_dir_all(&launch_agents_dir) {
                eprintln!("Failed to create LaunchAgents directory: {}", e);
                return;
            }
        }

        let plist_path = launch_agents_dir.join("com.bearice.rustcat.plist");

        if enable {
            if let Ok(exe_path) = std::env::current_exe() {
                let plist_content = format!(
                    r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.bearice.rustcat</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
    <key>LSUIElement</key>
    <true/>
</dict>
</plist>"#,
                    exe_path.display()
                );

                if let Err(e) = fs::write(&plist_path, plist_content) {
                    eprintln!("Failed to write launch agent plist: {}", e);
                }
            }
        } else {
            if let Err(e) = fs::remove_file(&plist_path) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    eprintln!("Failed to remove launch agent plist: {}", e);
                }
            }
        }
    }

    fn is_dark_mode_enabled() -> bool {
        let output = Command::new("defaults")
            .args(&["read", "-g", "AppleInterfaceStyle"])
            .output();

        if let Ok(output) = output {
            let result = String::from_utf8_lossy(&output.stdout);
            result.trim() == "Dark"
        } else {
            false
        }
    }

    fn migrate_legacy_settings() {
        // No legacy settings to migrate on macOS
    }
}

fn get_preference(key: &str) -> Option<String> {
    let output = Command::new("defaults")
        .args(&["read", "com.bearice.rustcat", key])
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            let result = String::from_utf8_lossy(&output.stdout);
            Some(result.trim().to_string())
        } else {
            None
        }
    } else {
        None
    }
}

fn set_preference(key: &str, value: &str) {
    let _ = Command::new("defaults")
        .args(&["write", "com.bearice.rustcat", key, value])
        .status();
}

fn remove_preference(key: &str) {
    let _ = Command::new("defaults")
        .args(&["delete", "com.bearice.rustcat", key])
        .status();
}
