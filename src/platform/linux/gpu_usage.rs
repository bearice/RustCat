use crate::platform::GpuMonitor;
use std::fs;
use std::io;
use std::process::Command;

pub struct LinuxGpuMonitor;

impl GpuMonitor for LinuxGpuMonitor {
    fn get_gpu_usage() -> io::Result<f64> {
        let sysfs = sysfs_gpu_usage();
        let nvidia = nvidia_smi_gpu_usage();

        match (sysfs, nvidia) {
            (Some(a), Some(b)) => Ok(a.max(b)),
            (Some(a), None) | (None, Some(a)) => Ok(a),
            (None, None) => Err(io::Error::other(
                "No GPU usage source found (no /sys/class/drm/*/device/gpu_busy_percent and nvidia-smi unavailable)",
            )),
        }
    }
}

/// amdgpu / i915 (and other DRM drivers) expose a per-card busy percentage
/// directly in sysfs — no helper tool needed. Returns the max across cards.
fn sysfs_gpu_usage() -> Option<f64> {
    let entries = fs::read_dir("/sys/class/drm").ok()?;
    let mut max: Option<f64> = None;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("card") {
            continue;
        }
        let path = format!("/sys/class/drm/{name}/device/gpu_busy_percent");
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(v) = content.trim().parse::<f64>() {
                max = Some(max.map_or(v, |m| m.max(v)));
            }
        }
    }
    max
}

/// NVIDIA GPUs have no sysfs busy percentage — query `nvidia-smi` instead.
/// Returns the max utilization across all NVIDIA GPUs.
fn nvidia_smi_gpu_usage() -> Option<f64> {
    let output = Command::new("nvidia-smi")
        .args(["--query-gpu=utilization.gpu", "--format=csv,noheader,nounits"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.trim().parse::<f64>().ok())
        .max_by(f64::total_cmp)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test: exercise the sampling path and print the reading so it
    /// can be checked manually on a machine with a GPU. Deliberately
    /// lenient — a machine with no GPU at all is not a failure.
    #[test]
    fn gpu_usage_smoke() {
        match LinuxGpuMonitor::get_gpu_usage() {
            Ok(v) => eprintln!("Linux GPU usage: {:.2}%", v),
            Err(e) => eprintln!("Linux GPU usage unavailable: {}", e),
        }
    }
}
