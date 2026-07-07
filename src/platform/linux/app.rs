use crate::app::App;
use crate::icon_manager::{IconManager, Theme};

pub struct LinuxApp {
    app: App,
}

impl LinuxApp {
    pub fn new(
        icon_manager: IconManager,
        initial_icon: &str,
        initial_theme: Option<Theme>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let app = App::new(icon_manager, initial_icon, initial_theme)?;
        Ok(LinuxApp { app })
    }

    pub fn start_animation_thread(&self) {
        self.app.start_animation_thread();
    }

    pub fn run(self) {
        // The trayicon Linux/KDE backend spawns its own background thread that
        // drives the D-Bus StatusNotifierItem protocol and forwards menu/click
        // events into our mpsc channel. So we just consume events on the main
        // thread; when Exit is requested the loop breaks and the process exits.
        self.app.run();
    }
}