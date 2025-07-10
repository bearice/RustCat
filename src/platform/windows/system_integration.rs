use crate::platform::SystemIntegration;
use std::process::Command;
use windows::{core::HSTRING, Win32::{Foundation::HWND, UI::WindowsAndMessaging::{MessageBoxW, MB_OK, MESSAGEBOX_STYLE}, System::SystemInformation::GetLocalTime}};

pub struct WindowsSystemIntegration;

impl SystemIntegration for WindowsSystemIntegration {
    fn show_dialog(
        message: &str,
        title: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        safe_message_box(message, title, MB_OK.0)?;
        Ok(())
    }

    fn open_system_monitor() -> Result<(), Box<dyn std::error::Error>> {
        Command::new("taskmgr").spawn()?;
        Ok(())
    }

    fn get_local_hour() -> u32 {
        unsafe {
            let st = GetLocalTime();
            st.wHour as u32
        }
    }
}

fn safe_message_box(
    message: &str,
    title: &str,
    flags: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    unsafe {
        let result = MessageBoxW(
            Some(HWND::default()),
            &HSTRING::from(message),
            &HSTRING::from(title),
            MESSAGEBOX_STYLE(flags),
        );
        if result.0 == 0 {
            return Err("MessageBoxW failed".to_string().into());
        }
    }
    Ok(())
}
