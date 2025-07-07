use trayicon::MenuBuilder;
use crate::settings::{is_run_on_start_enabled, is_dark, is_cat};

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Events {
    Exit,
    ThemeDark,
    ThemeLight,
    IconCat,
    IconParrot,
    RunTaskmgr,
    ToggleRunOnStart,
    ShowAboutDialog,
    ShowMenu,
}

pub fn build_menu(icon_id: usize) -> MenuBuilder<Events> {
    let run_on_start_enabled = is_run_on_start_enabled();
    MenuBuilder::new()
        .submenu(
            "&Theme",
            MenuBuilder::new()
                .checkable("&Dark", is_dark(icon_id), Events::ThemeDark)
                .checkable("&Light", !is_dark(icon_id), Events::ThemeLight),
        )
        .submenu(
            "&Icon",
            MenuBuilder::new()
                .checkable("&Cat", is_cat(icon_id), Events::IconCat)
                .checkable("&Parrot", !is_cat(icon_id), Events::IconParrot),
        )
        .separator()
        .checkable(
            "&Run on Start",
            run_on_start_enabled,
            Events::ToggleRunOnStart,
        )
        .separator()
        .item("&About", Events::ShowAboutDialog)
        .separator()
        .item("E&xit", Events::Exit)
}