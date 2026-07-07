use crate::platform::CpuMonitor;
use std::fs;
use std::io;
use std::sync::Mutex;

pub struct LinuxCpuMonitor;

static CPU_STATE: Mutex<Option<(f64, f64)>> = Mutex::new(None);

impl CpuMonitor for LinuxCpuMonitor {
    fn get_cpu_usage() -> io::Result<f64> {
        // /proc/stat first line (aggregate over all CPUs):
        //   cpu  user nice system idle iowait irq softirq steal guest guest_nice
        let contents = fs::read_to_string("/proc/stat")?;
        let first_line = contents
            .lines()
            .next()
            .ok_or_else(|| io::Error::other("Empty /proc/stat"))?;

        let fields: Vec<&str> = first_line.split_whitespace().collect();
        if fields.is_empty() || fields[0] != "cpu" {
            return Err(io::Error::other("Unexpected /proc/stat format"));
        }

        // Parse the time fields (user, nice, system, idle, iowait, irq, softirq, steal, ...)
        let ticks: Vec<f64> = fields[1..]
            .iter()
            .map(|f| f.parse::<f64>().unwrap_or(0.0))
            .collect();

        // idle = idle + iowait (indices 3 and 4)
        let idle = ticks.get(3).copied().unwrap_or(0.0)
            + ticks.get(4).copied().unwrap_or(0.0);
        let total: f64 = ticks.iter().sum();

        let mut state = CPU_STATE.lock().unwrap();
        let usage = if let Some((prev_total, prev_idle)) = *state {
            let total_diff = total - prev_total;
            let idle_diff = idle - prev_idle;
            if total_diff > 0.0 {
                // Clamp to [0, 100] — /proc counters can be non-monotonic on
                // VMs / after suspend, yielding spurious negative or >100 values.
                (100.0 - (idle_diff / total_diff * 100.0)).clamp(0.0, 100.0)
            } else {
                0.0
            }
        } else {
            0.0 // First call, return 0 usage
        };

        *state = Some((total, idle));
        Ok(usage)
    }
}