use crate::platform::{GpuDevice, GpuMonitor};
use std::alloc::{alloc, dealloc, Layout};
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::ptr::NonNull;
use std::sync::Mutex;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIAdapter1, IDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE,
};
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

/// A heap-allocated byte buffer with an explicit alignment, for passing to
/// `PdhGetFormattedCounterArrayW`. PDH fills the buffer with a leading array
/// of `PDH_FMT_COUNTERVALUE_ITEM_W` structs (each holding a `PWSTR` and an
/// `f64`), which require 8-byte alignment; a plain `Vec<u8>` only guarantees
/// byte alignment, so casting its pointer to that slice type would be
/// undefined behavior. Allocating through the global allocator with an
/// explicit `Layout` gives the buffer the alignment the item type needs.
struct AlignedBuf {
    ptr: NonNull<u8>,
    len: usize,
}

impl AlignedBuf {
    fn new(len: usize) -> io::Result<Self> {
        let layout = Layout::from_size_align(
            len,
            std::mem::align_of::<PDH_FMT_COUNTERVALUE_ITEM_W>(),
        )
        .map_err(|_| io::Error::other("invalid PDH array buffer layout"))?;
        let ptr = unsafe { alloc(layout) };
        if ptr.is_null() {
            return Err(io::Error::other(
                "out of memory allocating PDH array buffer",
            ));
        }
        Ok(AlignedBuf {
            ptr: unsafe { NonNull::new_unchecked(ptr) },
            len,
        })
    }

    fn as_mut_ptr(&mut self) -> *mut u8 {
        self.ptr.as_ptr()
    }

    fn as_ptr(&self) -> *const u8 {
        self.ptr.as_ptr()
    }
}

impl Drop for AlignedBuf {
    fn drop(&mut self) {
        // The same size/alignment pair succeeded at allocation time, so this
        // cannot fail.
        if let Ok(layout) = Layout::from_size_align(
            self.len,
            std::mem::align_of::<PDH_FMT_COUNTERVALUE_ITEM_W>(),
        ) {
            unsafe { dealloc(self.ptr.as_ptr(), layout) };
        }
    }
}

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
    /// Physical engine identity: `win-gpu-<luid>/<eng>` (or the full
    /// instance name when the fields are missing).
    engine_key: String,
    value: f64,
}

/// Parse a hex u32 like `0x00017FEA` (the `0x` prefix is optional).
fn parse_hex_u32(s: &str) -> Option<u32> {
    let s = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    u32::from_str_radix(s, 16).ok()
}

/// Parse an instance name like
/// `pid_10584_luid_0x00000000_0x00017FEA_phys_0_eng_0_engtype_3D`.
///
/// Returns `(gpu_id, gpu_label, engine_key)`. The physical GPU is
/// identified by the `luid` field, which is the adapter LUID — the same
/// value DXGI reports as `IDXGIAdapter::GetDesc1().AdapterLuid`, so the
/// id matches `enumerate_dxgi_adapters()` exactly (the `phys` field is NOT
/// a reliable per-adapter index: on multi-GPU systems it can be `phys_0`
/// for every adapter). Missing fields degrade gracefully so unusual names
/// still produce a usable (unique) key.
fn parse_instance_name(name: &str) -> (String, String, String) {
    let parts: Vec<&str> = name.split('_').collect();
    let mut luid: Option<(u32, u32)> = None; // (HighPart, LowPart)
    let mut eng: Option<&str> = None;
    let mut i = 0;
    while i < parts.len() {
        match parts[i] {
            // `luid_0x<HighPart>_0x<LowPart>` — the two following tokens are
            // the adapter LUID halves.
            "luid" if i + 2 < parts.len() => {
                if let (Some(h), Some(l)) =
                    (parse_hex_u32(parts[i + 1]), parse_hex_u32(parts[i + 2]))
                {
                    luid = Some((h, l));
                }
                i += 2;
            }
            "eng" if i + 1 < parts.len() => {
                eng = Some(parts[i + 1]);
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }

    match (luid, eng) {
        (Some((h, l)), Some(e)) => (
            format!("win-gpu-{h:08x}_{l:08x}"),
            format!("GPU 0x{l:08x}"),
            format!("win-gpu-{h:08x}_{l:08x}/{e}"),
        ),
        (Some((h, l)), None) => (
            format!("win-gpu-{h:08x}_{l:08x}"),
            format!("GPU 0x{l:08x}"),
            format!("win-gpu-{h:08x}_{l:08x}"),
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
        let mut buf = AlignedBuf::new(size as usize)?;
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
///
/// `known` is the set of GPU ids that currently exist (installed display
/// adapters plus any with live instances). A scope that is in `known` but has
/// no live instances is an *idle* GPU and reports 0%, rather than being
/// treated as a missing device.
fn aggregate(
    instances: &[EngineInstance],
    scope: Option<&str>,
    known: &BTreeSet<String>,
) -> io::Result<f64> {
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
        // No live instances in scope. If the scope is a known (idle) device,
        // report 0%; otherwise it no longer exists.
        match scope {
            Some(id) if known.contains(id) => return Ok(0.0),
            Some(_) => {
                return Err(io::Error::other(
                    "Selected GPU device no longer exists (fall back to all GPUs)",
                ))
            }
            None => {
                // All GPUs idle (or none with live data).
                if !known.is_empty() {
                    return Ok(0.0);
                }
                return Err(io::Error::other(
                    "No GPU engine utilization data available",
                ));
            }
        }
    }
    engines
        .values()
        .copied()
        .max_by(f64::total_cmp)
        .ok_or_else(|| io::Error::other("No GPU engine utilization data available"))
}

/// Enumerate every installed GPU adapter via DXGI, independently of the live
/// PDH utilization instances. This is what makes idle (no process contexts)
/// secondary GPUs show up in the device menu.
///
/// `IDXGIFactory1::EnumAdapters1` yields exactly one adapter per physical
/// GPU (unlike `EnumDisplayDevices`, which fans out per display output).
/// Each adapter's id is its LUID (`win-gpu-<HighPart>_<LowPart>`), the same
/// value that appears in the PDH instance names, so the ids match exactly
/// for scoping. The adapter description is used as the display name.
///
/// Returns the real (hardware) adapters plus the set of software adapter ids
/// (e.g. "Microsoft Basic Render Driver"), which the caller can use to keep
/// the software fallbacks out of the device menu.
fn enumerate_dxgi() -> (Vec<(String, String)>, BTreeSet<String>) {
    let mut real: Vec<(String, String)> = Vec::new();
    let mut software: BTreeSet<String> = BTreeSet::new();
    let factory: IDXGIFactory1 = match unsafe { CreateDXGIFactory1() } {
        Ok(f) => f,
        Err(_) => return (real, software),
    };
    let mut index = 0u32;
    loop {
        // `EnumAdapters1` returns `Err` (DXGI_ERROR_NOT_FOUND) past the last
        // adapter.
        let adapter: IDXGIAdapter1 = match unsafe { factory.EnumAdapters1(index) } {
            Ok(a) => a,
            Err(_) => break,
        };
        if let Ok(d) = unsafe { adapter.GetDesc1() } {
            let id = format!(
                "win-gpu-{:08x}_{:08x}",
                d.AdapterLuid.HighPart, d.AdapterLuid.LowPart
            );
            // Software fallback adapters are tracked separately.
            if d.Flags & (DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) != 0 {
                software.insert(id);
            } else {
                let end = d
                    .Description
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(d.Description.len());
                let name = String::from_utf16_lossy(&d.Description[..end]);
                let name = if name.trim().is_empty() {
                    format!("GPU 0x{:08x}", d.AdapterLuid.LowPart)
                } else {
                    name
                };
                real.push((id, name));
            }
        }
        index += 1;
    }
    (real, software)
}

impl GpuMonitor for WindowsGpuMonitor {
    fn enumerate_gpus() -> Vec<GpuDevice> {
        let (real, software) = enumerate_dxgi();
        let mut devices: BTreeMap<String, String> = BTreeMap::new();
        // Installed adapters first (includes idle ones).
        for (id, name) in real {
            devices.insert(id, name);
        }
        // Active PDH instances catch GPUs not in the display list (e.g.
        // headless compute adapters); their label is used only if the
        // display enumeration did not already provide a name. Software
        // fallback adapters are skipped to keep the menu clean.
        if let Ok(instances) = collect_instances() {
            for inst in &instances {
                if !software.contains(&inst.gpu_id) {
                    devices
                        .entry(inst.gpu_id.clone())
                        .or_insert_with(|| inst.gpu_label.clone());
                }
            }
        }
        devices
            .into_iter()
            .map(|(id, name)| GpuDevice { id, name })
            .collect()
    }

    fn get_gpu_usage(scope: Option<&str>) -> io::Result<f64> {
        let instances = collect_instances()?;
        // The set of known GPU ids: installed display adapters plus any that
        // currently have PDH instances. A scoped id in this set but without
        // instances is an *idle* GPU (report 0%), not a missing one.
        let mut known: BTreeSet<String> = BTreeSet::new();
        for (id, _) in enumerate_dxgi().0 {
            known.insert(id);
        }
        for inst in &instances {
            known.insert(inst.gpu_id.clone());
        }
        aggregate(&instances, scope, &known)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_name_full() {
        // The id is the adapter LUID (matches DXGI's AdapterLuid), NOT the
        // `phys` field (which is `phys_0` for every adapter on some systems).
        let (gpu_id, gpu_label, engine_key) =
            parse_instance_name("pid_10584_luid_0x00000000_0x00017FEA_phys_0_eng_0_engtype_3D");
        assert_eq!(gpu_id, "win-gpu-00000000_00017fea");
        assert_eq!(gpu_label, "GPU 0x00017fea");
        assert_eq!(engine_key, "win-gpu-00000000_00017fea/0");
    }

    #[test]
    fn parse_name_multiple_gpus_and_engtypes() {
        // Non-zero luid halves, high engine index, engtype with spaces.
        let (gpu_id, gpu_label, engine_key) =
            parse_instance_name("pid_1_luid_0x1_0x2_phys_1_eng_10_engtype_Video Codec 0");
        assert_eq!(gpu_id, "win-gpu-00000001_00000002");
        assert_eq!(gpu_label, "GPU 0x00000002");
        assert_eq!(engine_key, "win-gpu-00000001_00000002/10");
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
        let gpus = WindowsGpuMonitor::enumerate_gpus();
        eprintln!("Windows GPUs: {:?}", gpus);
        match WindowsGpuMonitor::get_gpu_usage(None) {
            Ok(v) => eprintln!("Windows GPU usage (all): {:.2}%", v),
            Err(e) => eprintln!("Windows GPU usage unavailable: {}", e),
        }
        // Scope to each enumerated GPU to verify the id matches PDH.
        for g in &gpus {
            match WindowsGpuMonitor::get_gpu_usage(Some(&g.id)) {
                Ok(v) => eprintln!("  scoped {}: {:.2}%", g.name, v),
                Err(e) => eprintln!("  scoped {}: error: {}", g.name, e),
            }
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
        // The set of GPU ids that currently exist.
        let known: BTreeSet<String> = ["g".to_string(), "other".to_string()]
            .into_iter()
            .collect();
        let instances = vec![
            inst("1", "e0", 35.0),
            inst("2", "e0", 45.0), // same engine, second process → 80
            inst("1", "e1", 10.0), // other engine, must not win
        ];
        let v = aggregate(&instances, None, &known).unwrap();
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
        assert_eq!(aggregate(&instances, None, &known).unwrap(), 100.0);

        // Scope filters by GPU id (the 60% instance of the other GPU is
        // ignored).
        let mut other = inst("1", "e9", 50.0);
        other.gpu_id = "other".into();
        let mut instances = capped;
        instances.push(other);
        assert_eq!(aggregate(&instances, Some("g"), &known).unwrap(), 60.0);
        // A scope that is not a known device is missing.
        assert!(aggregate(&instances, Some("missing"), &known).is_err());

        // A known device with no live instances is idle → 0%, not missing.
        let idle = vec![EngineInstance {
            gpu_id: "other".into(),
            gpu_label: "GPU 1".into(),
            engine_key: "other/e0".into(),
            value: 42.0,
        }];
        assert_eq!(aggregate(&idle, Some("g"), &known).unwrap(), 0.0);
        // No scope and no live instances but known devices exist → 0%.
        assert_eq!(aggregate(&[], None, &known).unwrap(), 0.0);
        // No scope, no instances, no known devices → error.
        let empty: BTreeSet<String> = BTreeSet::new();
        assert!(aggregate(&[], None, &empty).is_err());
    }
}
