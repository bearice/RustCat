use crate::platform::SystemIntegration;
use std::process::Command;

pub struct MacosSystemIntegration;

impl SystemIntegration for MacosSystemIntegration {
    fn show_dialog(message: &str, title: &str) -> Result<(), Box<dyn std::error::Error>> {
        Command::new("osascript")
            .arg("-e")
            .arg(&format!(
                r#"display dialog "{}" with title "{}" buttons {{"OK"}} default button "OK""#,
                message.replace("\"", "\\\""),
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
}
