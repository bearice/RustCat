use crate::platform::{GpuDevice, GpuMonitor};
use objc2_core_foundation::{CFDictionary, CFNumber, CFString, CFType, CFRetained};
use std::ffi::{c_void, CString};
use std::io;
use std::ptr::NonNull;
use std::sync::Mutex;

type MachPort = u32;

/// The default (current) Mach port; IOKit interprets 0 as "this process".
const K_IO_MAIN_PORT_DEFAULT: MachPort = 0;

/// A GPU service we track: its IORegistry handle, a stable-per-boot id for
/// menu selection, and a human-readable name.
#[derive(Clone)]
struct GpuEntry {
    service: u32,
    id: String,
    name: String,
}

// Service handles kept in GPU_ENTRIES are cached for the process lifetime
// and never released; services we skip are released as we go.
extern "C" {
    fn IOServiceMatching(name: *const i8) -> *mut c_void;
    fn IOServiceGetMatchingServices(
        main_port: MachPort,
        matching: *mut c_void,
        existing: *mut u32,
    ) -> u32;
    fn IOIteratorNext(iterator: u32) -> u32;
    fn IOObjectRelease(object: u32);
    fn IORegistryEntryCreateCFProperty(
        entry: u32,
        key: *const c_void,
        allocator: *const c_void,
        options: u32,
    ) -> *const c_void;
}

/// Cached list of GPU services. IOAccelerator/AGXAccelerator services are
/// looked up once and reused; a failed property read invalidates the cache
/// so a hot-plugged GPU is picked up on the next sample.
static GPU_ENTRIES: Mutex<Option<Vec<GpuEntry>>> = Mutex::new(None);

pub struct MacosGpuMonitor;

impl GpuMonitor for MacosGpuMonitor {
    fn enumerate_gpus() -> Vec<GpuDevice> {
        gpu_entries()
            .into_iter()
            .map(|e| GpuDevice {
                id: e.id,
                name: e.name,
            })
            .collect()
    }

    fn get_gpu_usage(scope: Option<&str>) -> io::Result<f64> {
        let mut entries = gpu_entries();
        if entries.is_empty() {
            return Err(io::Error::other(
                "No GPU performance statistics available (no IOAccelerator service found)",
            ));
        }
        if scope.map_or(false, |s| !entries.iter().any(|e| e.id == s)) {
            return Err(io::Error::other(
                "Selected GPU device no longer exists (fall back to all GPUs)",
            ));
        }
        if let Some(v) = read_max(&entries, scope) {
            return Ok(v);
        }

        // All reads failed — the cached services may be stale (e.g. after a
        // GPU hot-plug). Invalidate the cache and retry once.
        *GPU_ENTRIES.lock().unwrap() = None;
        entries = gpu_entries();
        if entries.is_empty() {
            return Err(io::Error::other(
                "No GPU performance statistics available (no IOAccelerator service found)",
            ));
        }
        if scope.map_or(false, |s| !entries.iter().any(|e| e.id == s)) {
            return Err(io::Error::other(
                "Selected GPU device no longer exists (fall back to all GPUs)",
            ));
        }
        match read_max(&entries, scope) {
            Some(v) => Ok(v),
            None => Err(io::Error::other(
                "GPU service found but no 'Device Utilization %' statistic",
            )),
        }
    }
}

/// The maximum "Device Utilization %" across the entries in scope
/// (`scope` = `None` means all entries), or `None` when no entry exposes
/// the statistic.
fn read_max(entries: &[GpuEntry], scope: Option<&str>) -> Option<f64> {
    let mut max: Option<f64> = None;
    for e in entries {
        if let Some(scope) = scope {
            if e.id != scope {
                continue;
            }
        }
        if let Some(v) = read_device_utilization(e.service) {
            max = Some(max.map(|m| m.max(v)).unwrap_or(v));
        }
    }
    max
}

/// Return the cached GPU service list, looking it up on first use.
fn gpu_entries() -> Vec<GpuEntry> {
    let mut guard = GPU_ENTRIES.lock().unwrap();
    if guard.is_none() {
        *guard = Some(lookup_gpu_entries());
    }
    guard.clone().unwrap_or_default()
}

/// Enumerate every GPU accelerator service.
///
/// NOTE: the dictionary returned by `IOServiceMatching` must NOT be released
/// by us — `IOServiceGetMatchingServices` is declared `CF_RELEASES_ARGUMENT`
/// and always consumes one reference of the matching dictionary (observed
/// 2026-09-11: CFReleasing it afterwards segfaults).
fn lookup_gpu_entries() -> Vec<GpuEntry> {
    for class in ["IOAccelerator", "AGXAccelerator"] {
        let Ok(class_cstr) = CString::new(class) else {
            continue;
        };
        let matching_raw = unsafe { IOServiceMatching(class_cstr.as_ptr()) };
        if matching_raw.is_null() {
            continue;
        }
        let mut iterator: u32 = 0;
        let ret = unsafe {
            IOServiceGetMatchingServices(K_IO_MAIN_PORT_DEFAULT, matching_raw, &mut iterator)
        };
        if ret != 0 || iterator == 0 {
            // kIOReturnSuccess == 0; a NULL iterator means "no matches".
            continue;
        }

        let mut entries = Vec::new();
        let mut service = unsafe { IOIteratorNext(iterator) };
        while service != 0 {
            // Only keep services that actually expose the utilization
            // statistic (matches the single-service behavior). Kept
            // services are cached for the process lifetime and never
            // released; the rest are released right away.
            if read_device_utilization(service).is_some() {
                let name = read_string_property(service, "model")
                    .or_else(|| read_string_property(service, "name"))
                    .unwrap_or_else(|| format!("GPU {}", entries.len()));
                let id = format!("macos-gpu-{}", entries.len());
                entries.push(GpuEntry {
                    service,
                    id,
                    name,
                });
            } else {
                unsafe { IOObjectRelease(service) };
            }
            service = unsafe { IOIteratorNext(iterator) };
        }
        // Release the iterator; the services we kept are cached.
        unsafe { IOObjectRelease(iterator) };

        if !entries.is_empty() {
            return entries;
        }
    }
    Vec::new()
}

/// Read "Device Utilization %" from the IORegistry `PerformanceStatistics`
/// property of the given service.
///
/// This is the same data source `powermetrics` uses, and it is readable
/// without root privileges. Returns `None` when the service does not
/// expose the key.
fn read_device_utilization(service: u32) -> Option<f64> {
    let prop_key = CFString::from_str("PerformanceStatistics");
    let prop_raw = unsafe {
        IORegistryEntryCreateCFProperty(
            service,
            CFRetained::as_ptr(&prop_key).as_ptr() as *const c_void,
            std::ptr::null(),
            0,
        )
    };
    if prop_raw.is_null() {
        return None;
    }
    // Take ownership of the property (create rule).
    let prop = unsafe { CFRetained::from_raw(NonNull::new_unchecked(prop_raw as *mut CFType)) };

    let dict = prop.downcast::<CFDictionary>().ok()?;
    // Cast to the concrete key/value types we expect.
    let dict = unsafe { CFRetained::cast_unchecked::<CFDictionary<CFString, CFType>>(dict) };

    let stat_key = CFString::from_str("Device Utilization %");
    let value = dict.get(&stat_key)?;
    let number = value.downcast::<CFNumber>().ok()?;
    number.as_f64()
}

/// Read a string property (e.g. `model`) from the given service.
fn read_string_property(service: u32, key: &str) -> Option<String> {
    let prop_key = CFString::from_str(key);
    let prop_raw = unsafe {
        IORegistryEntryCreateCFProperty(
            service,
            CFRetained::as_ptr(&prop_key).as_ptr() as *const c_void,
            std::ptr::null(),
            0,
        )
    };
    if prop_raw.is_null() {
        return None;
    }
    let prop = unsafe { CFRetained::from_raw(NonNull::new_unchecked(prop_raw as *mut CFType)) };
    let s = prop.downcast::<CFString>().ok()?;
    Some(s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test: exercise the sampling path and print the reading so it
    /// can be checked manually on a machine with a GPU. Deliberately
    /// lenient — a machine with no GPU at all is not a failure.
    #[test]
    fn gpu_usage_smoke() {
        eprintln!("macOS GPUs: {:?}", MacosGpuMonitor::enumerate_gpus());
        match MacosGpuMonitor::get_gpu_usage(None) {
            Ok(v) => eprintln!("macOS GPU usage: {:.2}%", v),
            Err(e) => eprintln!("macOS GPU usage unavailable: {}", e),
        }
    }
}
