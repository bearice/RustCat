use crate::app::App;
use crate::icon_manager::{IconManager, Theme};
use objc2::rc::Retained;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
use objc2_foundation::MainThreadMarker;
use std::sync::Arc;

pub struct MacosApp {
    app: Arc<App>,
    ns_app: Retained<NSApplication>,
}

impl MacosApp {
    pub fn new(
        icon_manager: IconManager,
        initial_icon: &str,
        initial_theme: Option<Theme>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Initialize NSApplication for macOS
        let mtm = unsafe { MainThreadMarker::new_unchecked() };
        let ns_app = NSApplication::sharedApplication(mtm);
        ns_app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

        let app = Arc::new(App::new(icon_manager, initial_icon, initial_theme)?);
        Ok(MacosApp { app, ns_app })
    }

    pub fn start_animation_thread(&self) {
        self.app.start_animation_thread();
    }

    pub fn run(self) {
        // Start the app event handler in a separate thread
        let app = Arc::try_unwrap(self.app).unwrap_or_else(|_| {
            panic!("Failed to unwrap Arc<App> - multiple references exist");
        });
        std::thread::spawn(move || {
            app.run();
        });

        // Run the macOS application main loop
        self.ns_app.run();
    }
}
