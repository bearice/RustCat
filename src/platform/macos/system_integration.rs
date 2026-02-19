use crate::platform::SystemIntegration;
use std::process::Command;

pub struct MacosSystemIntegration;

impl SystemIntegration for MacosSystemIntegration {
    fn show_dialog(message: &str, title: &str) -> Result<(), Box<dyn std::error::Error>> {
        Command::new("osascript")
            .arg("-e")
            .arg(&format!(
                r#"display dialog "{}" with title "{}" buttons {{"OK"}} default button "OK""#,
                message.replace("\\", "\\\\").replace("\"", "\\\""),
                title.replace("\\", "\\\\").replace("\"", "\\\"")
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
        let output = Command::new("date")
            .arg("+%H")
            .output()
            .unwrap_or_else(|_| std::process::Output {
                status: std::process::ExitStatus::default(),
                stdout: b"0".to_vec(),
                stderr: Vec::new(),
            });
        
        String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .unwrap_or(0)
    }
}
