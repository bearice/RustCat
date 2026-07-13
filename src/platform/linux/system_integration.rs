use crate::platform::SystemIntegration;
use std::process::Command;

pub struct LinuxSystemIntegration;

impl SystemIntegration for LinuxSystemIntegration {
    fn show_dialog(message: &str, title: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Prefer KDE's kdialog, fall back to zenity, then xmessage.
        //
        // We block on the child's exit status rather than fire-and-forget
        // spawning: a tool may spawn successfully yet exit non-zero (e.g.
        // kdialog with no display/D-Bus session), and we must fall through to
        // the next tool instead of silently succeeding with nothing shown.
        // Blocking is fine here — the tray animation runs on its own thread,
        // so only menu event processing pauses while this modal dialog is up.
        // status() also reaps the child, so no zombie accumulates.
        let mut kdialog = Command::new("kdialog");
        kdialog
            .arg("--title")
            .arg(title)
            .arg("--msgbox")
            .arg(message);
        if run_dialog(&mut kdialog) {
            return Ok(());
        }
        let mut zenity = Command::new("zenity");
        zenity.args(["--title", title, "--info", "--text", message]);
        if run_dialog(&mut zenity) {
            return Ok(());
        }
        let mut xmessage = Command::new("xmessage");
        xmessage.args(["-title", title, message]);
        if run_dialog(&mut xmessage) {
            return Ok(());
        }

        // Last resort: print to stderr so the message is at least visible
        // somewhere (journal / terminal that launched the app).
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

/// Run a modal dialog tool to completion and return `true` if it displayed
/// successfully. Returns `false` both when the tool is not installed (spawn
/// failed) and when it spawned but exited non-zero (e.g. no display) so the
/// caller can fall through to the next tool. `status()` reaps the child.
fn run_dialog(cmd: &mut Command) -> bool {
    match cmd.status() {
        Ok(status) => status.success(),
        Err(_) => false,
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