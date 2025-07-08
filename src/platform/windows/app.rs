use crate::app::App;
use crate::icon_manager::{IconManager, Theme};
use crate::platform::{PlatformSettingsManager, SettingsManager};

pub struct WindowsApp {
    app: App,
}

impl WindowsApp {
    pub fn new(
        icon_manager: IconManager,
        initial_icon: &str,
        initial_theme: Option<Theme>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let app = App::new(icon_manager, initial_icon, initial_theme)?;
        Ok(WindowsApp { app })
    }

    pub fn start_animation_thread(&self) {
        self.app.start_animation_thread();
    }

    pub fn run(self) {
        self.app.run();
    }

    pub fn shutdown(&self) {
        self.app.shutdown();
    }
}
