use crate::platform::CpuMonitor;
use std::io;
use std::sync::Mutex;
use windows::{Win32::Foundation::FILETIME, Win32::System::Threading::GetSystemTimes};

pub struct WindowsCpuMonitor;

static CPU_STATE: Mutex<Option<(f64, f64)>> = Mutex::new(None);

impl CpuMonitor for WindowsCpuMonitor {
    fn get_cpu_usage() -> io::Result<f64> {
        let mut idle_time = empty();
        let mut kernel_time = empty();
        let mut user_time = empty();
        unsafe {
            GetSystemTimes(
                Some(&mut idle_time),
                Some(&mut kernel_time),
                Some(&mut user_time),
            )
            .map_err(|e| io::Error::other(format!("Failed to get system times: {}", e)))?;
        }
        let idle_time = filetime_to_u64(idle_time) as f64;
        let kernel_time = filetime_to_u64(kernel_time) as f64;
        let user_time = filetime_to_u64(user_time) as f64;
        let total_time = kernel_time + user_time;
        
        let mut state = CPU_STATE.lock().unwrap();
        let usage = if let Some((prev_total, prev_idle)) = *state {
            let total_diff = total_time - prev_total;
            let idle_diff = idle_time - prev_idle;
            if total_diff > 0.0 {
                100.0 - (idle_diff / total_diff * 100.0)
            } else {
                0.0
            }
        } else {
            0.0 // First call, return 0 usage
        };
        
        *state = Some((total_time, idle_time));
        Ok(usage)
    }
}

/// Essentailly a no-op
#[inline(always)]
fn filetime_to_u64(f: FILETIME) -> u64 {
    (f.dwHighDateTime as u64) << 32 | (f.dwLowDateTime as u64)
}

/// Empty FILETIME
#[inline(always)]
fn empty() -> FILETIME {
    FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    }
}
