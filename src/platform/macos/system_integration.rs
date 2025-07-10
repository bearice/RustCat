use crate::platform::SystemIntegration;
use objc2_foundation::{NSCalendar, NSCalendarUnit, NSDate};
use std::process::Command;

pub struct MacosSystemIntegration;

impl SystemIntegration for MacosSystemIntegration {
    fn show_dialog(message: &str, title: &str) -> Result<(), Box<dyn std::error::Error>> {
        Command::new("osascript")
            .arg("-e")
            .arg(&format!(
                r#"display dialog "{}" with title "{}" buttons {{"OK"}} default button "OK""#,
                message.replace("\"", "\\\"").replace("\n", "\\n"),
                title.replace("\"", "\\\"")
            ))
            .spawn()?;
        Ok(())
    }

    fn open_system_monitor() -> Result<(), Box<dyn std::error::Error>> {
        Command::new("open")
            .arg("-a")
            .arg("Activity Monitor")
            .spawn()?;
        Ok(())
    }

    fn get_local_hour() -> u32 {
        unsafe {
            let calendar = NSCalendar::currentCalendar();
            let now = NSDate::now();
            let components = calendar.components_fromDate(NSCalendarUnit::Hour, &now);
            components.hour() as u32
        }
    }
}
