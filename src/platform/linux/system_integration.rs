use crate::platform::SystemIntegration;
use std::process::Command;

pub struct LinuxSystemIntegration;

impl SystemIntegration for LinuxSystemIntegration {
    fn show_dialog(message: &str, title: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Prefer KDE's kdialog, fall back to zenity, then xmessage.
        if try_spawn(Command::new("kdialog").arg("--title").arg(title).arg("--msgbox").arg(message)) {
            return Ok(());
        }
        if try_spawn(Command::new("zenity").args(["--title", title, "--info", "--text", message])) {
            return Ok(());
        }
        if try_spawn(Command::new("xmessage").args(["-title", title, message])) {
            return Ok(());
        }
        // Last resort: just print to stderr
        eprintln!("{}: {}", title, message);
        Ok(())
    }

    fn open_system_monitor() -> Result<(), Box<dyn std::error::Error>> {
        // KDE system monitor (Plasma 5.21+), fall back to older ksysguard / htop.
        for prog in ["plasma-systemmonitor", "ksysguard", "gnome-system-monitor"] {
            if try_spawn(&mut Command::new(prog)) {
                return Ok(());
            }
        }
        if try_spawn(Command::new("xterm").arg("-e").arg("htop")) {
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

/// Spawn a command detached and reap it on a background thread so the child
/// does not become a zombie once it exits (RustCat is long-lived and would
/// otherwise accumulate one zombie per opened dialog / monitor).
fn try_spawn(cmd: &mut Command) -> bool {
    match cmd.spawn() {
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            true
        }
        Err(_) => false,
    }
}