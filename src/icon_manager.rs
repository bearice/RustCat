use trayicon::Icon;
use std::collections::HashMap;
use flate2::read::GzDecoder;
use std::io::Read;

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
}

impl Theme {
    pub fn from_system() -> Self {
        if crate::settings::is_dark_mode_enabled() {
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
        decoder.read_to_end(&mut all_decompressed)
            .map_err(|e| format!("Failed to decompress all icons: {}", e))?;
        
        // Leak the decompressed data to keep it alive for the lifetime of the program
        let all_decompressed = Box::leak(all_decompressed.into_boxed_slice());
        
        // Get icon metadata from build script generated module
        let icon_metadata_map = icon_data::get_icon_metadata();
        
        for (icon_name, theme_data) in icon_metadata_map {
            // All current icons support themes
            manager.theme_support.insert(icon_name.to_string(), true);
            let mut themes_map = HashMap::new();
            
            for (theme_str, group_info) in theme_data {
                let theme = match theme_str {
                    "dark" => Theme::Dark,
                    "light" => Theme::Light,
                    _ => continue, // Skip unknown themes
                };
                
                // Extract this group's data from the big decompressed chunk
                let mut icons = Vec::new();
                let mut current_offset = group_info.offset;
                
                for &size in group_info.sizes {
                    let size = size as usize;
                    if current_offset + size > all_decompressed.len() {
                        return Err(format!("Invalid icon offset/size data for {} {}", icon_name, theme_str).into());
                    }
                    
                    let icon_data = &all_decompressed[current_offset..current_offset + size];
                    
                    let icon = Icon::from_buffer(icon_data, None, None)
                        .map_err(|e| format!("Failed to create icon from buffer for {} {}: {}", icon_name, theme_str, e))?;
                    
                    icons.push(icon);
                    current_offset += size;
                }
                
                themes_map.insert(theme, icons);
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
                icon_map.get(&system_theme)
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
        let mut icons: Vec<String> = self.icon_sets.keys().cloned().collect();
        icons.sort();
        icons
    }
    
    pub fn available_themes_for_icon(&self, icon_name: &str) -> Vec<Theme> {
        if let Some(icon_map) = self.icon_sets.get(icon_name) {
            let mut themes: Vec<Theme> = icon_map.keys().copied().collect();
            themes.sort_by_key(|t| match t {
                Theme::Dark => 0,
                Theme::Light => 1,
            });
            themes
        } else {
            vec![]
        }
    }
    
    // Migration support - convert old numeric IDs to string IDs
    pub fn migrate_from_numeric_id(old_id: usize) -> String {
        let is_cat = (old_id & 2) == 0;
        
        if is_cat {
            "cat".to_string()
        } else {
            "parrot".to_string()
        }
    }
    
    pub fn get_theme_from_numeric_id(old_id: usize) -> Theme {
        let is_dark = (old_id & 1) == 0;
        if is_dark { Theme::Dark } else { Theme::Light }
    }
    
}

impl Default for IconManager {
    fn default() -> Self {
        Self::new()
    }
}