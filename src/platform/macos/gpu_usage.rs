use crate::platform::GpuMonitor;
use objc2_core_foundation::{CFDictionary, CFNumber, CFString, CFType, CFRetained};
use std::ffi::{c_void, CString};
use std::io;
use std::ptr::NonNull;
use std::sync::Mutex;

type MachPort = u32;

/// The default (current) Mach port; IOKit interprets 0 as "this process".
const K_IO_MAIN_PORT_DEFAULT: MachPort = 0;

// The service handle returned by IOServiceGetMatchingService is cached for
// the process lifetime (see GPU_SERVICE), so it is never released.
extern "C" {
    fn IOServiceMatching(name: *const i8) -> *mut c_void;
    fn IOServiceGetMatchingService(main_port: MachPort, matching: *mut c_void) -> u32;
    fn IORegistryEntryCreateCFProperty(
        entry: u32,
        key: *const c_void,
        allocator: *const c_void,
        options: u32,
    ) -> *const c_void;
}

/// Cached IORegistry service handle for the GPU. The service number is
/// stable for the process lifetime, so it is looked up once and reused.
static GPU_SERVICE: Mutex<Option<u32>> = Mutex::new(None);

pub struct MacosGpuMonitor;

impl GpuMonitor for MacosGpuMonitor {
    fn get_gpu_usage() -> io::Result<f64> {
        let service = get_gpu_service();
        if service == 0 {
            return Err(io::Error::other(
                "No GPU performance statistics available (no IOAccelerator service found)",
            ));
        }
        match read_device_utilization(service) {
            Some(usage) => Ok(usage),
            None => {
                // The property read failed — the cached service may be
                // stale (e.g. after a GPU hot-plug). Invalidate the cache
                // and retry once.
                *GPU_SERVICE.lock().unwrap() = None;
                let service = get_gpu_service();
                if service == 0 {
                    return Err(io::Error::other(
                        "No GPU performance statistics available (no IOAccelerator service found)",
                    ));
                }
                match read_device_utilization(service) {
                    Some(usage) => Ok(usage),
                    None => Err(io::Error::other(
                        "GPU service found but no 'Device Utilization %' statistic",
                    )),
                }
            }
        }
    }
}

/// Return the cached GPU service handle, looking it up on first use.
fn get_gpu_service() -> u32 {
    let mut guard = GPU_SERVICE.lock().unwrap();
    if guard.is_none() {
        *guard = Some(lookup_gpu_service());
    }
    guard.unwrap_or(0)
}

/// Find the first service exposing GPU performance statistics.
///
/// NOTE: the dictionary returned by `IOServiceMatching` must NOT be
/// released by us after `IOServiceGetMatchingService` — IOKit takes
/// ownership of it during the lookup (observed 2026-09-11: CFReleasing it
/// afterwards segfaults, and the property dictionary is allocated at the
/// matching dictionary's freed address).
fn lookup_gpu_service() -> u32 {
    for class in ["IOAccelerator", "AGXAccelerator"] {
        let Ok(class_cstr) = CString::new(class) else {
            continue;
        };
        let matching_raw = unsafe { IOServiceMatching(class_cstr.as_ptr()) };
        if matching_raw.is_null() {
            continue;
        }
        let service = unsafe { IOServiceGetMatchingService(K_IO_MAIN_PORT_DEFAULT, matching_raw) };
        if service != 0 && read_device_utilization(service).is_some() {
            return service;
        }
    }
    0
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test: exercise the sampling path and print the reading so it
    /// can be checked manually on a machine with a GPU. Deliberately
    /// lenient — a machine with no GPU at all is not a failure.
    #[test]
    fn gpu_usage_smoke() {
        match MacosGpuMonitor::get_gpu_usage() {
            Ok(v) => eprintln!("macOS GPU usage: {:.2}%", v),
            Err(e) => eprintln!("macOS GPU usage unavailable: {}", e),
        }
    }
}
