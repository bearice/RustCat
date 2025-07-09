use crate::icon_manager::{IconManager, Theme};
use crate::platform::{SettingsManager, SettingsManagerImpl};
use trayicon::MenuBuilder;

#[derive(Clone, Eq, PartialEq, Debug)]
pub enum Events {
    Exit,
    SetTheme(Theme),
    SetIcon(String),
    RunTaskmgr,
    ToggleRunOnStart,
    ShowAboutDialog,
    ShowMenu,
}

pub fn build_menu(icon_manager: &IconManager) -> MenuBuilder<Events> {
    let run_on_start_enabled = SettingsManagerImpl::is_run_on_start_enabled();
    let current_icon = SettingsManagerImpl::get_current_icon();
    let current_theme = SettingsManagerImpl::get_current_theme();

    let mut menu = MenuBuilder::new();

    // Build theme submenu - only show if current icon supports themes
    if icon_manager.supports_themes(&current_icon) {
        let available_themes = icon_manager.available_themes_for_icon(&current_icon);
        if !available_themes.is_empty() {
            let mut theme_menu = MenuBuilder::new();
            for theme in available_themes {
                let theme_name = match theme {
                    Theme::Dark => "Dark",
                    Theme::Light => "Light",
                    #[cfg(target_os = "macos")]
                    Theme::Auto => "Auto",
                };
                let is_current = current_theme == theme;
                println!("current_theme: {:?}, new_theme: {:?}", current_theme, theme);
                theme_menu = theme_menu.checkable(theme_name, is_current, Events::SetTheme(theme));
            }
            menu = menu.submenu("Theme", theme_menu);
        }
    }

    // Build icon submenu - dynamically from available icons
    let available_icons = icon_manager.available_icons();
    if available_icons.len() > 1 {
        let mut icon_menu = MenuBuilder::new();
        for icon_name in available_icons {
            let is_current = current_icon == icon_name;
            // Capitalize first letter for display
            let display_name = icon_name
                .chars()
                .next()
                .unwrap()
                .to_uppercase()
                .collect::<String>()
                + &icon_name[1..];
            icon_menu = icon_menu.checkable(&display_name, is_current, Events::SetIcon(icon_name));
        }
        menu = menu.submenu("Icon", icon_menu);
    }

    menu.separator()
        .checkable(
            "Run on Start",
            run_on_start_enabled,
            Events::ToggleRunOnStart,
        )
        .separator()
        .item("System Monitor", Events::RunTaskmgr)
        .item("About", Events::ShowAboutDialog)
        .separator()
        .item("Exit", Events::Exit)
}
