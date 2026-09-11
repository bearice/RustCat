use crate::platform::{GpuDevice, GpuMonitor};
use std::fs;
use std::io;
use std::process::Command;

pub struct LinuxGpuMonitor;

/// A device id + name + reading from one utilization source.
struct GpuEntry {
    id: String,
    name: String,
    value: f64,
}

impl GpuMonitor for LinuxGpuMonitor {
    fn enumerate_gpus() -> Vec<GpuDevice> {
        let mut devices: Vec<GpuDevice> = Vec::new();
        for entry in sysfs_gpu_entries() {
            devices.push(GpuDevice {
                id: entry.id,
                name: entry.name,
            });
        }
        for entry in nvidia_smi_entries() {
            devices.push(GpuDevice {
                id: entry.id,
                name: entry.name,
            });
        }
        devices
    }

    fn get_gpu_usage(scope: Option<&str>) -> io::Result<f64> {
        let mut entries = Vec::new();
        entries.extend(sysfs_gpu_entries());
        entries.extend(nvidia_smi_entries());

        if entries.is_empty() {
            return Err(io::Error::other(
                "No GPU usage source found (no /sys/class/drm/*/device/gpu_busy_percent and nvidia-smi unavailable)",
            ));
        }
        if scope.map_or(false, |s| !entries.iter().any(|e| e.id == s)) {
            return Err(io::Error::other(
                "Selected GPU device no longer exists (fall back to all GPUs)",
            ));
        }
        let mut max: Option<f64> = None;
        for e in &entries {
            if let Some(scope) = scope {
                if e.id != scope {
                    continue;
                }
            }
            max = Some(max.map_or(e.value, |m| m.max(e.value)));
        }
        max.ok_or_else(|| {
            io::Error::other("No GPU usage source found (no /sys/class/drm/*/device/gpu_busy_percent and nvidia-smi unavailable)")
        })
    }
}

/// amdgpu (and a few other DRM drivers) expose a per-card busy percentage
/// directly in sysfs — no helper tool needed. Note: i915 does NOT expose
/// `gpu_busy_percent`; Intel utilization requires the i915 perf/PMU
/// interface, which needs perf_event permissions a tray app cannot rely on,
/// so on Intel-only systems this source simply finds no cards.
/// Returns one entry per card.
fn sysfs_gpu_entries() -> Vec<GpuEntry> {
    let mut entries = Vec::new();
    let Ok(dir) = fs::read_dir("/sys/class/drm") else {
        return entries;
    };
    for entry in dir.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Some(index) = name.strip_prefix("card") else {
            continue;
        };
        let path = format!("/sys/class/drm/{name}/device/gpu_busy_percent");
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(v) = content.trim().parse::<f64>() {
                entries.push(GpuEntry {
                    id: format!("card{index}"),
                    name: format!("GPU card{index}"),
                    value: v,
                });
            }
        }
    }
    entries
}

/// NVIDIA GPUs have no sysfs busy percentage — query `nvidia-smi` instead,
/// asking for the per-GPU index, name and utilization in one CSV pass.
/// Returns one entry per NVIDIA GPU.
fn nvidia_smi_entries() -> Vec<GpuEntry> {
    let mut entries = Vec::new();
    let Ok(output) = Command::new("nvidia-smi")
        .args(["--query-gpu=index,name,utilization.gpu", "--format=csv,noheader,nounits"])
        .output()
    else {
        return entries;
    };
    if !output.status.success() {
        return entries;
    }
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        // Rows look like: `0, NVIDIA GeForce RTX 3080, 12`
        let mut parts = line.splitn(3, ',');
        let (Some(index), Some(name), Some(util)) =
            (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let Ok(v) = util.trim().parse::<f64>() else {
            continue;
        };
        let index = index.trim();
        entries.push(GpuEntry {
            id: format!("nvidia-{index}"),
            name: name.trim().to_string(),
            value: v,
        });
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test: exercise the sampling path and print the reading so it
    /// can be checked manually on a machine with a GPU. Deliberately
    /// lenient — a machine with no GPU at all is not a failure.
    #[test]
    fn gpu_usage_smoke() {
        eprintln!("Linux GPUs: {:?}", LinuxGpuMonitor::enumerate_gpus());
        match LinuxGpuMonitor::get_gpu_usage(None) {
            Ok(v) => eprintln!("Linux GPU usage: {:.2}%", v),
            Err(e) => eprintln!("Linux GPU usage unavailable: {}", e),
        }
    }
}
