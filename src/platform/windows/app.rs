use windows::Win32::UI::WindowsAndMessaging::{DispatchMessageW, GetMessageW, TranslateMessage};

use crate::app::App;
use crate::icon_manager::{IconManager, Theme};

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
        let exit_flag = self.app.exit_flag.clone();
        std::thread::spawn(move || {
            self.app.run();
        });
        while !exit_flag.load(std::sync::atomic::Ordering::Relaxed) {
            unsafe {
            let mut msg = std::mem::zeroed();
            let bret = GetMessageW(&mut msg, None, 0, 0);
                if bret.as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                } else {
                    break;
                }
            }
        }
    }
}
