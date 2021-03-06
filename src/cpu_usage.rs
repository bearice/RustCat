use std::io;

use winapi::shared::minwindef::FILETIME;
use winapi::um::processthreadsapi::GetSystemTimes;

pub fn get_cpu_totals() -> io::Result<(f64, f64)> {
    let mut idle_time = empty();
    let mut kernel_time = empty();
    let mut user_time = empty();
    unsafe {
        if GetSystemTimes(&mut idle_time, &mut kernel_time, &mut user_time) == 0 {
            panic!("Error getting cpu usage");
        }
    }
    let idle_time = filetime_to_u64(idle_time) as f64;
    let kernel_time = filetime_to_u64(kernel_time) as f64;
    let user_time = filetime_to_u64(user_time) as f64;
    let total_time = kernel_time + user_time;
    Ok((total_time, idle_time))
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
