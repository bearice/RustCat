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

pub fn is_dark(id: usize) -> bool {
    id & 1 == 0
}

pub fn is_cat(id: usize) -> bool {
    id & 2 == 0
}

pub fn get_icon_id() -> usize {
    let key = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(sub_key) = key.open_subkey_with_flags("Software\\RustCat", KEY_READ) {
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

pub fn set_icon_id(id: usize) {
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
        .set_value("IconId", &(id as u32))
        .expect("set_value");
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