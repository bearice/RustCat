use crate::platform::CpuMonitor;
use std::io;
use std::sync::Mutex;

pub struct MacosCpuMonitor;

static CPU_STATE: Mutex<Option<(f64, f64)>> = Mutex::new(None);

// macOS system types
#[repr(C)]
struct HostCpuLoadInfo {
    cpu_ticks: [u32; 4], // CPU_STATE_USER, CPU_STATE_SYSTEM, CPU_STATE_IDLE, CPU_STATE_NICE
}

const CPU_STATE_USER: usize = 0;
const CPU_STATE_SYSTEM: usize = 1;
const CPU_STATE_IDLE: usize = 2;
const CPU_STATE_NICE: usize = 3;

const HOST_CPU_LOAD_INFO: i32 = 3;
const HOST_CPU_LOAD_INFO_COUNT: u32 = 4;

extern "C" {
    fn host_statistics(
        host_priv: u32,
        flavor: i32,
        host_info_out: *mut HostCpuLoadInfo,
        host_info_outCnt: *mut u32,
    ) -> i32;

    fn mach_host_self() -> u32;
}

impl CpuMonitor for MacosCpuMonitor {
    fn get_cpu_usage() -> io::Result<f64> {
        let mut cpu_info = HostCpuLoadInfo { cpu_ticks: [0; 4] };
        let mut count = HOST_CPU_LOAD_INFO_COUNT;

        let result = unsafe {
            host_statistics(
                mach_host_self(),
                HOST_CPU_LOAD_INFO,
                &mut cpu_info as *mut HostCpuLoadInfo,
                &mut count,
            )
        };

        if result != 0 {
            return Err(io::Error::other(format!(
                "Failed to get CPU statistics: {}",
                result
            )));
        }

        let user_ticks = cpu_info.cpu_ticks[CPU_STATE_USER] as f64;
        let system_ticks = cpu_info.cpu_ticks[CPU_STATE_SYSTEM] as f64;
        let idle_ticks = cpu_info.cpu_ticks[CPU_STATE_IDLE] as f64;
        let nice_ticks = cpu_info.cpu_ticks[CPU_STATE_NICE] as f64;

        let total_ticks = user_ticks + system_ticks + idle_ticks + nice_ticks;

        let mut state = CPU_STATE.lock().unwrap();
        let usage = if let Some((prev_total, prev_idle)) = *state {
            let total_diff = total_ticks - prev_total;
            let idle_diff = idle_ticks - prev_idle;
            if total_diff > 0.0 {
                100.0 - (idle_diff / total_diff * 100.0)
            } else {
                0.0
            }
        } else {
            0.0 // First call, return 0 usage
        };

        *state = Some((total_ticks, idle_ticks));
        Ok(usage)
    }
}
