use trayicon::Icon;
use std::collections::HashMap;

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
        
        // Define icon configurations: (base_name, supports_themes, [(theme, icon_data)])
        // Adding new icons is as simple as adding a new entry here!
        let icon_configs = [
            ("cat", true, vec![
                (Theme::Dark, crate::icon_data::DARK_CAT),
                (Theme::Light, crate::icon_data::LIGHT_CAT),
            ]),
            ("parrot", true, vec![
                (Theme::Dark, crate::icon_data::DARK_PARROT),
                (Theme::Light, crate::icon_data::LIGHT_PARROT),
            ]),
            // Example: to add a new "dog" icon, just add:
            // ("dog", true, vec![
            //     (Theme::Dark, crate::icon_data::DARK_DOG),
            //     (Theme::Light, crate::icon_data::LIGHT_DOG),
            // ]),
            // The menu system will automatically detect it and add it to the menu!
        ];
        
        for (base_name, supports_themes, theme_data) in icon_configs.iter() {
            manager.theme_support.insert(base_name.to_string(), *supports_themes);
            let mut themes_map = HashMap::new();
            
            for (theme, data) in theme_data.iter() {
                let icons: Result<Vec<Icon>, _> = data
                    .iter()
                    .map(|icon_bytes| Icon::from_buffer(icon_bytes, None, None))
                    .collect();
                
                themes_map.insert(*theme, icons?);
            }
            
            manager.icon_sets.insert(base_name.to_string(), themes_map);
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