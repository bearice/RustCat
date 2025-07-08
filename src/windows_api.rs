use windows::{
    core::*,
    Win32::Foundation::HWND,
    Win32::UI::Shell::*,
    Win32::UI::WindowsAndMessaging::*,
};

pub fn safe_message_box(message: &str, title: &str, flags: u32) -> std::result::Result<(), Box<dyn std::error::Error>> {
    unsafe {
        let result = MessageBoxW(
            Some(HWND::default()),
            &HSTRING::from(message),
            &HSTRING::from(title),
            MESSAGEBOX_STYLE(flags),
        );
        if result.0 == 0 {
            return Err("MessageBoxW failed".into());
        }
    }
    Ok(())
}

pub fn safe_shell_execute(file: &str) -> std::result::Result<(), Box<dyn std::error::Error>> {
    unsafe {
        let ret = ShellExecuteW(
            Some(HWND::default()),
            None,
            &HSTRING::from(file),
            None,
            None,
            SW_SHOWNORMAL,
        );
        if ret.0 as usize <= 32 {
            return Err(format!("ShellExecute failed with code: {}", ret.0 as usize).into());
        }
    }
    Ok(())
}

pub fn safe_message_loop() -> std::result::Result<(), Box<dyn std::error::Error>> {
    unsafe {
        let mut msg = std::mem::zeroed();
        let bret = GetMessageA(&mut msg, None, 0, 0);
        if bret.as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageA(&msg);
            Ok(())
        } else {
            Err("GetMessageA returned 0 (WM_QUIT)".into())
        }
    }
}