use flate2::read::GzDecoder;
use std::collections::HashMap;
use std::io::Read;
use trayicon::Icon;

#[allow(dead_code)]
mod icon_data {
    pub struct IconGroupInfo {
        pub offset: usize,
        pub sizes: &'static [u32],
    }
    pub type IconData = HashMap<&'static str, HashMap<&'static str, IconGroupInfo>>;
    include!(concat!(env!("OUT_DIR"), "/icon_data.rs"));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Theme {
    Dark,
    Light,
    #[cfg(target_os = "macos")]
    Auto,
}

impl Theme {
    #[cfg(target_os = "macos")]
    pub fn from_system() -> Self {
        Theme::Auto
    }
    #[cfg(not(target_os = "macos"))]
    pub fn from_system() -> Self {
        use crate::platform::{SettingsManager, SettingsManagerImpl};
        if SettingsManagerImpl::is_dark_mode_enabled() {
            Theme::Dark
        } else {
            Theme::Light
        }
    }
}

impl std::fmt::Display for Theme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Theme::Dark => write!(f, "dark"),
            Theme::Light => write!(f, "light"),
            #[cfg(target_os = "macos")]
            Theme::Auto => write!(f, "auto"),
        }
    }
}

pub struct IconManager {
    // Maps base icon name -> theme -> icon frames
    icon_sets: HashMap<String, HashMap<Theme, Vec<Icon>>>,
    // Maps base icon name -> whether it supports themes
    theme_support: HashMap<String, bool>,
}

impl IconManager {
    pub fn new() -> Self {
        Self {
            icon_sets: HashMap::new(),
            theme_support: HashMap::new(),
        }
    }

    pub fn load_icons() -> Result<Self, Box<dyn std::error::Error>> {
        let mut manager = Self::new();

        // Decompress the single big chunk containing all icons
        let mut decoder = GzDecoder::new(icon_data::ALL_ICONS_COMPRESSED);
        let mut all_decompressed = Vec::new();
        decoder
            .read_to_end(&mut all_decompressed)
            .map_err(|e| format!("Failed to decompress all icons: {}", e))?;

        // Leak the decompressed data to keep it alive for the lifetime of the program
        let all_decompressed = Box::leak(all_decompressed.into_boxed_slice());

        // Get icon metadata from build script generated module
        let icon_metadata_map = icon_data::get_icon_metadata();

        fn load_icons(
            icon_name: &str,
            theme_str: &str,
            data: &'static [u8],
            offset: usize,
            sizes: &[u32],
        ) -> Result<Vec<Icon>, String> {
            let mut icons = Vec::new();
            let mut current_offset = offset as usize;

            for &size in sizes {
                let size = size as usize;
                if current_offset + size > data.len() {
                    return Err(format!(
                        "Invalid icon offset/size data: {} + {} exceeds data length",
                        current_offset, size
                    ));
                }

                let icon_data = &data[current_offset..current_offset + size];
                let icon = {
                    #[cfg(windows)]
                    {
                        Icon::from_buffer(icon_data, None, None).map_err(|e| {
                            format!(
                                "Failed to create icon from buffer for {} {}: {}",
                                icon_name, theme_str, e
                            )
                        })
                    }
                    #[cfg(target_os = "macos")]
                    {
                        // macOS tray icons should be smaller (16x16 is the standard size)
                        Icon::from_buffer(icon_data, Some(16), Some(16)).map_err(|e| {
                            format!(
                                "Failed to create icon from buffer for {} {}: {}",
                                icon_name, theme_str, e
                            )
                        })
                    }
                }?;

                icons.push(icon);
                current_offset += size;
            }

            Ok(icons)
        }

        for (icon_name, theme_data) in icon_metadata_map {
            // All current icons support themes
            manager.theme_support.insert(icon_name.to_string(), true);
            let mut themes_map = HashMap::new();

            for (&theme_str, group_info) in &theme_data {
                let theme = match theme_str {
                    "dark" => Theme::Dark,
                    "light" => Theme::Light,
                    _ => continue, // Skip unknown themes
                };
                let icons = load_icons(
                    icon_name,
                    theme_str,
                    all_decompressed,
                    group_info.offset,
                    group_info.sizes,
                )?;

                themes_map.insert(theme, icons);
            }

            #[cfg(target_os = "macos")]
            {
                // macOS icons are always themed because Auto are generated from first theme
                let group_info = theme_data
                    .get("light")
                    .or_else(|| theme_data.get("dark"))
                    .unwrap();
                let mut icons = load_icons(
                    icon_name,
                    "auto",
                    all_decompressed,
                    group_info.offset,
                    group_info.sizes,
                )?;
                for icon in &mut icons {
                    icon.set_template(true);
                }
                manager.theme_support.insert(icon_name.to_string(), true);
                themes_map.insert(Theme::Auto, icons);
            }
            manager.icon_sets.insert(icon_name.to_string(), themes_map);
        }

        Ok(manager)
    }

    pub fn get_icon_set(&self, icon_name: &str, theme: Option<Theme>) -> Option<&Vec<Icon>> {
        let icon_map = self.icon_sets.get(icon_name)?;

        if let Some(theme) = theme {
            // Specific theme requested
            icon_map.get(&theme)
        } else {
            // Auto-detect theme or use first available
            if self.supports_themes(icon_name) {
                let system_theme = Theme::from_system();
                icon_map
                    .get(&system_theme)
                    .or_else(|| icon_map.values().next()) // Fallback to any theme
            } else {
                icon_map.values().next() // Single theme icon
            }
        }
    }

    pub fn supports_themes(&self, icon_name: &str) -> bool {
        self.theme_support.get(icon_name).copied().unwrap_or(false)
    }

    pub fn available_icons(&self) -> Vec<String> {
        let mut icons: Vec<String> = self.icon_sets.keys()
            .filter(|&name| name != "sleep") // Hide sleep icons from menu
            .cloned()
            .collect();
        icons.sort();
        icons
    }

    pub fn available_themes_for_icon(&self, icon_name: &str) -> Vec<Theme> {
        if let Some(icon_map) = self.icon_sets.get(icon_name) {
            let mut themes: Vec<Theme> = icon_map.keys().copied().collect();
            themes.sort_by_key(|t| match t {
                Theme::Dark => 0,
                Theme::Light => 1,
                #[cfg(target_os = "macos")]
                Theme::Auto => 2,
            });
            themes
        } else {
            vec![]
        }
    }

    // Migration support - convert old numeric IDs to string IDs
    #[cfg(windows)]
    pub fn migrate_from_numeric_id(old_id: usize) -> String {
        let is_cat = (old_id & 2) == 0;

        if is_cat {
            "cat".to_string()
        } else {
            "parrot".to_string()
        }
    }

    #[cfg(windows)]
    pub fn get_theme_from_numeric_id(old_id: usize) -> Theme {
        let is_dark = (old_id & 1) == 0;
        if is_dark {
            Theme::Dark
        } else {
            Theme::Light
        }
    }
}

impl Default for IconManager {
    fn default() -> Self {
        Self::new()
    }
}
