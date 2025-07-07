use winreg::RegKey;
use winreg::enums::*;

pub fn is_run_on_start_enabled() -> bool {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(run_key) = hkcu.open_subkey_with_flags(
        "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
        KEY_READ,
    ) {
        if run_key.get_value::<String, _>("RustCat").is_ok() {
            return true;
        }
    }
    false
}

pub fn set_run_on_start(enable: bool) {
    use std::env;

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

use crate::icon_manager::{IconManager, Theme};

// Backward compatibility functions for numeric IDs
pub fn is_dark(id: usize) -> bool {
    IconManager::is_dark_theme_numeric(id)
}

pub fn is_cat(id: usize) -> bool {
    IconManager::is_cat_type_numeric(id)
}

// String-based icon ID functions
pub fn get_current_icon() -> String {
    let key = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(sub_key) = key.open_subkey_with_flags("Software\\RustCat", KEY_READ) {
        // Try to get string-based icon name first
        if let Ok(icon_name) = sub_key.get_value::<String, &str>("IconName") {
            return icon_name;
        }
        
        // Migration: Check for old numeric IconId and convert
        if let Ok(old_id) = sub_key.get_value::<u32, &str>("IconId") {
            let migrated_name = IconManager::migrate_from_numeric_id(old_id as usize);
            
            // Save the migrated name and remove old numeric ID
            set_current_icon(&migrated_name);
            let _ = sub_key.delete_value("IconId"); // Ignore errors if already deleted
            
            return migrated_name;
        }
    }
    
    // Default icon
    "cat".to_string()
}

pub fn set_current_icon(icon_name: &str) {
    let key = RegKey::predef(HKEY_CURRENT_USER);
    let sub_key = if let Ok(sub_key) = key.open_subkey_with_flags(
        "Software\\RustCat",
        KEY_WRITE | KEY_READ,
    ) {
        sub_key
    } else {
        key.create_subkey_with_flags(
            "Software\\RustCat",
            KEY_WRITE | KEY_READ,
        )
        .expect("create_subkey_with_flags")
        .0
    };
    
    sub_key
        .set_value("IconName", &icon_name)
        .expect("set_value");
}

pub fn get_current_theme() -> Theme {
    let key = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(sub_key) = key.open_subkey_with_flags("Software\\RustCat", KEY_READ) {
        // Try to get explicit theme preference
        if let Ok(theme_str) = sub_key.get_value::<String, &str>("Theme") {
            match theme_str.as_str() {
                "dark" => return Theme::Dark,
                "light" => return Theme::Light,
                _ => {} // Fall through to auto-detect
            }
        }
        
        // Migration: Check for old numeric IconId theme
        if let Ok(old_id) = sub_key.get_value::<u32, &str>("IconId") {
            let theme = IconManager::get_theme_from_numeric_id(old_id as usize);
            // Save the migrated theme
            set_current_theme(Some(theme));
            return theme;
        }
    }
    
    // Auto-detect from system
    Theme::from_system()
}

pub fn set_current_theme(theme: Option<Theme>) {
    let key = RegKey::predef(HKEY_CURRENT_USER);
    let sub_key = if let Ok(sub_key) = key.open_subkey_with_flags(
        "Software\\RustCat",
        KEY_WRITE | KEY_READ,
    ) {
        sub_key
    } else {
        key.create_subkey_with_flags(
            "Software\\RustCat",
            KEY_WRITE | KEY_READ,
        )
        .expect("create_subkey_with_flags")
        .0
    };
    
    match theme {
        Some(theme) => {
            sub_key
                .set_value("Theme", &theme.to_string())
                .expect("set_value");
        }
        None => {
            // Auto-detect - remove explicit preference
            let _ = sub_key.delete_value("Theme"); // Ignore errors if doesn't exist
        }
    }
}

// Legacy functions for backward compatibility during transition
pub fn get_icon_id() -> usize {
    // Convert current string-based icon to old numeric format for compatibility
    let icon_name = get_current_icon();
    let theme = get_current_theme();
    
    let type_bit = match icon_name.as_str() {
        "cat" => 0,
        "parrot" => 2,
        _ => 0, // Default to cat
    };
    
    let theme_bit = match theme {
        Theme::Dark => 0,
        Theme::Light => 1,
    };
    
    type_bit | theme_bit
}

pub fn set_icon_id(id: usize) {
    // Convert old numeric ID to new string-based system
    let icon_name = IconManager::migrate_from_numeric_id(id);
    let theme = IconManager::get_theme_from_numeric_id(id);
    
    set_current_icon(&icon_name);
    set_current_theme(Some(theme));
}

pub fn is_dark_mode_enabled() -> bool {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
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