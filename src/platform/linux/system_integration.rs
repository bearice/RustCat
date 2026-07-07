use crate::platform::SystemIntegration;
use std::process::Command;

pub struct LinuxSystemIntegration;

impl SystemIntegration for LinuxSystemIntegration {
    fn show_dialog(message: &str, title: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Prefer KDE's kdialog, fall back to zenity, then xmessage.
        if Command::new("kdialog").arg("--title").arg(title).arg("--msgbox").arg(message).spawn().is_ok() {
            return Ok(());
        }
        if Command::new("zenity").args(["--title", title, "--info", "--text", message]).spawn().is_ok() {
            return Ok(());
        }
        if Command::new("xmessage").args(["-title", title, message]).spawn().is_ok() {
            return Ok(());
        }
        // Last resort: just print to stderr
        eprintln!("{}: {}", title, message);
        Ok(())
    }

    fn open_system_monitor() -> Result<(), Box<dyn std::error::Error>> {
        // KDE system monitor (Plasma 5.21+), fall back to older ksysguard / htop.
        for prog in ["plasma-systemmonitor", "ksysguard", "gnome-system-monitor"] {
            if Command::new(prog).spawn().is_ok() {
                return Ok(());
            }
        }
        if Command::new("xterm").arg("-e").arg("htop").spawn().is_ok() {
            return Ok(());
        }
        Err("No system monitor found (tried plasma-systemmonitor, ksysguard, gnome-system-monitor, htop)".into())
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