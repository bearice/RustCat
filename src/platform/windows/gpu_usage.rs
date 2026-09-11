use crate::platform::{GpuDevice, GpuMonitor};
use std::collections::BTreeMap;
use std::io;
use std::sync::Mutex;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::System::Performance::{
    PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY, PDH_MORE_DATA,
    PdhAddEnglishCounterW, PdhCollectQueryData, PdhCloseQuery, PdhGetFormattedCounterArrayW,
    PdhGetFormattedCounterValue, PdhOpenQueryW,
};

pub struct WindowsGpuMonitor;

/// PDH query state, opened lazily on the first sample and reused for the
/// lifetime of the process (a tray app never tears it down).
struct GpuPdhState {
    hquery: PDH_HQUERY,
    hcounter: PDH_HCOUNTER,
}

// The PDH handles are only ever touched from the animation thread, which
// owns the `Mutex` guard; the raw handles themselves are not Send.
unsafe impl Send for GpuPdhState {}

static GPU_STATE: Mutex<Option<GpuPdhState>> = Mutex::new(None);

/// Per-engine utilization counter, available since Windows 10 1803 for all
/// GPU vendors (NVIDIA, AMD, Intel). The `(*)` wildcard expands to one
/// instance per (process, engine) pair — instance names look like
/// `pid_10584_luid_0x00000000_0x00017FEA_phys_0_eng_0_engtype_3D` — so the
/// utilization of a physical engine is the SUM of its instances (one per
/// process context), and the overall GPU figure is the max across engines
/// (the busiest engine), mirroring how Task Manager surfaces "GPU busy".
const GPU_UTIL_COUNTER: PCWSTR =
    windows::core::w!("\\GPU Engine(*)\\Utilization Percentage");

/// One (process, engine) instance sample.
#[derive(Clone)]
struct EngineInstance {
    /// Physical GPU id from the instance name (`phys_<n>`), e.g.
    /// "win-gpu-0"; "default" when the name carries no phys field. Note:
    /// the `luid_...` token is a per-GPU-context id (a single physical
    /// GPU can have many luids), so it must NOT be used for the device
    /// identity.
    gpu_id: String,
    /// Menu label for the GPU (`GPU 0`, `GPU 1`, ...).
    gpu_label: String,
    /// Physical engine identity: `win-gpu-<phys>/<eng>` (or the full
    /// instance name when the fields are missing).
    engine_key: String,
    value: f64,
}

/// Parse an instance name like
/// `pid_10584_luid_0x00000000_0x00017FEA_phys_0_eng_0_engtype_3D`.
///
/// Returns `(gpu_id, gpu_label, engine_key)`. The physical GPU is
/// identified by the `phys` field (the `luid` field varies per GPU
/// context even on a single physical GPU). Missing fields degrade
/// gracefully so unusual names still produce a usable (unique) key.
fn parse_instance_name(name: &str) -> (String, String, String) {
    let parts: Vec<&str> = name.split('_').collect();
    let mut phys: Option<&str> = None;
    let mut eng: Option<&str> = None;
    let mut i = 0;
    while i < parts.len() {
        match parts[i] {
            "phys" if i + 1 < parts.len() => {
                phys = Some(parts[i + 1]);
                i += 1;
            }
            "eng" if i + 1 < parts.len() => {
                eng = Some(parts[i + 1]);
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }

    match (phys, eng) {
        (Some(p), Some(e)) => (
            format!("win-gpu-{p}"),
            format!("GPU {p}"),
            format!("win-gpu-{p}/{e}"),
        ),
        _ => ("default".to_string(), "GPU".to_string(), name.to_string()),
    }
}

/// Read a NUL-terminated UTF-16 string from a PDH item name pointer.
unsafe fn read_item_name(ptr: PWSTR) -> Option<String> {
    let start = ptr.as_ptr();
    if start.is_null() {
        return None;
    }
    let len = (0..4096).take_while(|&n| unsafe { *start.add(n) } != 0).count();
    if len == 0 {
        return None;
    }
    Some(unsafe { String::from_utf16_lossy(std::slice::from_raw_parts(start, len)) })
}

/// Open (or reuse) the PDH query and collect one sample of every
/// (process, engine) instance.
fn collect_instances() -> io::Result<Vec<EngineInstance>> {
    let mut state = GPU_STATE.lock().unwrap();
    if state.is_none() {
        let mut hquery = PDH_HQUERY::default();
        let ret = unsafe { PdhOpenQueryW(None::<&PCWSTR>, 0, &mut hquery) };
        if ret != 0 {
            return Err(io::Error::other(format!(
                "PdhOpenQueryW failed: 0x{:08X}",
                ret
            )));
        }

        let mut hcounter = PDH_HCOUNTER::default();
        let ret = unsafe { PdhAddEnglishCounterW(hquery, GPU_UTIL_COUNTER, 0, &mut hcounter) };
        if ret != 0 {
            unsafe {
                let _ = PdhCloseQuery(hquery);
            }
            return Err(io::Error::other(format!(
                "Counter not available (GPU counter missing on this system): 0x{:08X}",
                ret
            )));
        }

        // Prime the query: the first collect initializes the counters,
        // so the first real sample has a baseline.
        unsafe {
            let _ = PdhCollectQueryData(hquery);
        };

        *state = Some(GpuPdhState { hquery, hcounter });
    }

    let state = state.as_ref().expect("state checked above");
    let ret = unsafe { PdhCollectQueryData(state.hquery) };
    if ret == PDH_MORE_DATA {
        return Err(io::Error::other("PDH data not ready yet"));
    }
    if ret != 0 {
        return Err(io::Error::other(format!(
            "PdhCollectQueryData failed: 0x{:08X}",
            ret
        )));
    }

    // Wildcard counters expand into one instance per (process, engine);
    // the array API returns them all in one call. The first pass yields
    // the exact buffer size (struct array + trailing name storage), so
    // allocate `size` bytes and read exactly `count` items.
    let mut size: u32 = 0;
    let mut count: u32 = 0;
    let ret = unsafe {
        PdhGetFormattedCounterArrayW(state.hcounter, PDH_FMT_DOUBLE, &mut size, &mut count, None)
    };
    if ret == PDH_MORE_DATA && count > 0 {
        let mut buf: Vec<u8> = vec![0u8; size as usize];
        let ret = unsafe {
            PdhGetFormattedCounterArrayW(
                state.hcounter,
                PDH_FMT_DOUBLE,
                &mut size,
                &mut count,
                Some(buf.as_mut_ptr().cast()),
            )
        };
        if ret == 0 {
            let items = unsafe {
                std::slice::from_raw_parts(buf.as_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>(), count as usize)
            };
            let mut instances = Vec::new();
            for item in items {
                // PDH_CSTATUS_SUCCESS == 0; skip instances with no data.
                if item.FmtValue.CStatus != 0 {
                    continue;
                }
                let name = match unsafe { read_item_name(item.szName) } {
                    Some(n) => n,
                    None => continue,
                };
                let (gpu_id, gpu_label, engine_key) = parse_instance_name(&name);
                instances.push(EngineInstance {
                    gpu_id,
                    gpu_label,
                    engine_key,
                    value: unsafe { item.FmtValue.Anonymous.doubleValue },
                });
            }
            return Ok(instances);
        }
        Err(io::Error::other(format!(
            "PdhGetFormattedCounterArrayW failed: 0x{:08X}",
            ret
        )))
    } else if ret == 0 {
        // Single-instance counter (no wildcard expansion).
        let mut status: u32 = 0;
        let mut value = windows::Win32::System::Performance::PDH_FMT_COUNTERVALUE::default();
        let ret = unsafe {
            PdhGetFormattedCounterValue(
                state.hcounter,
                PDH_FMT_DOUBLE,
                Some(&mut status),
                &mut value,
            )
        };
        if ret == 0 && status == 0 {
            return Ok(vec![EngineInstance {
                gpu_id: "default".to_string(),
                gpu_label: "GPU".to_string(),
                engine_key: "default".to_string(),
                value: unsafe { value.Anonymous.doubleValue },
            }]);
        }
        Err(io::Error::other(format!(
            "PdhGetFormattedCounterValue failed: 0x{:08X} (status: {})",
            ret, status
        )))
    } else {
        Err(io::Error::other(format!(
            "PdhGetFormattedCounterArrayW unexpected status: 0x{:08X}",
            ret
        )))
    }
}

/// Sum per-process instances per physical engine (capped at 100%) and
/// return the max across engines for the requested GPU scope.
fn aggregate(instances: &[EngineInstance], scope: Option<&str>) -> io::Result<f64> {
    let mut engines: BTreeMap<(String, String), f64> = BTreeMap::new();
    let mut scoped = false;
    for inst in instances {
        if let Some(scope) = scope {
            if inst.gpu_id != scope {
                continue;
            }
        }
        scoped = true;
        let entry = engines
            .entry((inst.gpu_id.clone(), inst.engine_key.clone()))
            .or_insert(0.0);
        *entry = (*entry + inst.value).min(100.0);
    }
    if !scoped {
        return Err(io::Error::other(
            "Selected GPU device no longer exists (fall back to all GPUs)",
        ));
    }
    engines
        .values()
        .copied()
        .max_by(f64::total_cmp)
        .ok_or_else(|| io::Error::other("No GPU engine utilization data available"))
}

impl GpuMonitor for WindowsGpuMonitor {
    fn enumerate_gpus() -> Vec<GpuDevice> {
        let instances = match collect_instances() {
            Ok(i) => i,
            Err(_) => return Vec::new(),
        };
        let mut seen: BTreeMap<String, String> = BTreeMap::new();
        for inst in &instances {
            seen.entry(inst.gpu_id.clone())
                .or_insert_with(|| inst.gpu_label.clone());
        }
        seen
            .into_iter()
            .map(|(id, name)| GpuDevice { id, name })
            .collect()
    }

    fn get_gpu_usage(scope: Option<&str>) -> io::Result<f64> {
        let instances = collect_instances()?;
        aggregate(&instances, scope)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_name_full() {
        let (gpu_id, gpu_label, engine_key) =
            parse_instance_name("pid_10584_luid_0x00000000_0x00017FEA_phys_0_eng_0_engtype_3D");
        assert_eq!(gpu_id, "win-gpu-0");
        assert_eq!(gpu_label, "GPU 0");
        assert_eq!(engine_key, "win-gpu-0/0");
    }

    #[test]
    fn parse_name_multiple_gpus_and_engtypes() {
        // Second physical GPU, high engine index, engtype with spaces.
        let (gpu_id, gpu_label, engine_key) =
            parse_instance_name("pid_1_luid_0x1_0x2_phys_1_eng_10_engtype_Video Codec 0");
        assert_eq!(gpu_id, "win-gpu-1");
        assert_eq!(gpu_label, "GPU 1");
        assert_eq!(engine_key, "win-gpu-1/10");
    }

    #[test]
    fn parse_name_degenerate() {
        let (gpu_id, _label, engine_key) = parse_instance_name("3D 0");
        assert_eq!(gpu_id, "default");
        assert_eq!(engine_key, "3D 0");
    }

    /// Smoke test: exercise the sampling path and print the reading so it
    /// can be checked manually on a machine with a GPU. Deliberately
    /// lenient — a machine with no GPU at all is not a failure.
    #[test]
    fn gpu_usage_smoke() {
        eprintln!("Windows GPUs: {:?}", WindowsGpuMonitor::enumerate_gpus());
        match WindowsGpuMonitor::get_gpu_usage(None) {
            Ok(v) => eprintln!("Windows GPU usage: {:.2}%", v),
            Err(e) => eprintln!("Windows GPU usage unavailable: {}", e),
        }
    }

    #[test]
    fn aggregate_sums_per_engine_and_maxes() {
        let inst = |_pid: &str, eng: &str, v: f64| EngineInstance {
            gpu_id: "g".into(),
            gpu_label: "GPU 0".into(),
            engine_key: eng.into(),
            value: v,
        };
        let instances = vec![
            inst("1", "e0", 35.0),
            inst("2", "e0", 45.0), // same engine, second process → 80
            inst("1", "e1", 10.0), // other engine, must not win
        ];
        let v = aggregate(&instances, None).unwrap();
        assert_eq!(v, 80.0);

        // Sum is capped at 100.
        let capped = vec![EngineInstance {
            gpu_id: "g".into(),
            gpu_label: "GPU 0".into(),
            engine_key: "e0".into(),
            value: 60.0,
        }];
        let mut instances = capped.clone();
        instances.push(capped[0].clone());
        assert_eq!(aggregate(&instances, None).unwrap(), 100.0);

        // Scope filters by GPU id (the 60% instance of the other GPU is
        // ignored).
        let mut other = inst("1", "e9", 50.0);
        other.gpu_id = "other".into();
        let mut instances = capped;
        instances.push(other);
        assert_eq!(aggregate(&instances, Some("g")).unwrap(), 60.0);
        assert!(aggregate(&instances, Some("missing")).is_err());
    }
}
