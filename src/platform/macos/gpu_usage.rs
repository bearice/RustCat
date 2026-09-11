use crate::platform::{GpuDevice, GpuMonitor};
use objc2_core_foundation::{CFDictionary, CFNumber, CFString, CFType, CFRetained};
use std::collections::{HashMap, HashSet};
use std::ffi::{c_void, CString};
use std::io;
use std::ptr::NonNull;
use std::sync::{LazyLock, Mutex};

type MachPort = u32;

/// The default (current) Mach port; IOKit interprets 0 as "this process".
const K_IO_MAIN_PORT_DEFAULT: MachPort = 0;

/// A GPU service we track: its IORegistry handle, a device-derived id for
/// menu selection, and a human-readable name.
///
/// The id is derived from the IORegistry service handle itself (which is
/// stable for the device's lifetime — verified 2026-09-11: re-enumeration
/// returns the same handle), NOT from enumeration position. This means a
/// hot-plug/detach that changes the enumeration order cannot make a scoped
/// selection silently point at a different device: a removed device's id
/// simply disappears and triggers the missing-device fallback.
#[derive(Clone)]
struct GpuEntry {
    service: u32,
    id: String,
    name: String,
}

/// Reference counts for owned IOKit service handles. Every handle returned
/// by `IOIteratorNext` is retained here; a handle is `IOObjectRelease`d when
/// its count drops to zero. This is what lets us release superseded or
/// detached services on each periodic refresh (and on cache invalidation)
/// without leaking them for the life of the tray process, while still keeping
/// a handle alive for any in-flight reader that holds a reference to it.
static SERVICE_REFS: LazyLock<Mutex<HashMap<u32, u32>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn retain_service(service: u32) {
    let mut map = SERVICE_REFS.lock().unwrap();
    *map.entry(service).or_insert(0) += 1;
}

fn release_service(service: u32) {
    let mut map = SERVICE_REFS.lock().unwrap();
    match map.get_mut(&service) {
        Some(count) if *count > 1 => *count -= 1,
        Some(_) => {
            map.remove(&service);
            unsafe { IOObjectRelease(service) };
        }
        None => {
            // Not tracked (defensive); nothing to release.
        }
    }
}

fn retain_entries(entries: &[GpuEntry]) {
    for e in entries {
        retain_service(e.service);
    }
}

fn release_entries(entries: &[GpuEntry]) {
    for e in entries {
        release_service(e.service);
    }
}

/// RAII guard over a set of entries that owns one reference to each of their
/// service handles. `gpu_entries()` returns a clone with these references
/// already taken; the guard releases them on drop so callers never leak.
struct EntriesGuard {
    entries: Vec<GpuEntry>,
}

impl EntriesGuard {
    fn new(entries: Vec<GpuEntry>) -> Self {
        EntriesGuard { entries }
    }
    fn as_slice(&self) -> &[GpuEntry] {
        &self.entries
    }
}

impl Drop for EntriesGuard {
    fn drop(&mut self) {
        release_entries(&self.entries);
    }
}

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

/// Cached list of GPU services. IOAccelerator/AGXAccelerator services can
/// be hot-plugged (e.g. an eGPU attached after startup), so the list is
/// re-enumerated every `GPU_REFRESH_INTERVAL` samples and an empty result
/// is never cached (a later-attached GPU must be discoverable on the next
/// sample). A failed property read also invalidates the cache.
static GPU_CACHE: Mutex<Option<GpuCache>> = Mutex::new(None);

struct GpuCache {
    entries: Vec<GpuEntry>,
    samples_since_refresh: u32,
}

/// ~30 s at the app's 1 sample/s cadence.
const GPU_REFRESH_INTERVAL: u32 = 30;

pub struct MacosGpuMonitor;

impl GpuMonitor for MacosGpuMonitor {
    fn enumerate_gpus() -> Vec<GpuDevice> {
        let guard = EntriesGuard::new(gpu_entries());
        guard
            .as_slice()
            .iter()
            .map(|e| GpuDevice {
                id: e.id.clone(),
                name: e.name.clone(),
            })
            .collect()
    }

    fn get_gpu_usage(scope: Option<&str>) -> io::Result<f64> {
        let guard = EntriesGuard::new(gpu_entries());
        let entries = guard.as_slice();
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
        if let Some(v) = read_max(entries, scope) {
            return Ok(v);
        }

        // All reads failed — the cached services may be stale (e.g. after a
        // GPU hot-plug). Invalidate the cache (releasing its references) and
        // retry once.
        invalidate_gpu_cache();
        let guard2 = EntriesGuard::new(gpu_entries());
        let entries2 = guard2.as_slice();
        if entries2.is_empty() {
            return Err(io::Error::other(
                "No GPU performance statistics available (no IOAccelerator service found)",
            ));
        }
        if scope.map_or(false, |s| !entries2.iter().any(|e| e.id == s)) {
            return Err(io::Error::other(
                "Selected GPU device no longer exists (fall back to all GPUs)",
            ));
        }
        match read_max(entries2, scope) {
            Some(v) => Ok(v),
            None => Err(io::Error::other(
                "GPU service found but no 'Device Utilization %' statistic",
            )),
        }
    }
}

/// Release the references held by the current cache's entries and clear the
/// cache. In-flight readers are unaffected because their `EntriesGuard`
/// clones hold independent references.
fn invalidate_gpu_cache() {
    let mut guard = GPU_CACHE.lock().unwrap();
    if let Some(old) = guard.as_ref() {
        release_entries(&old.entries);
    }
    *guard = None;
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

/// Return the cached GPU service list, re-enumerating when due (first
/// use, every `GPU_REFRESH_INTERVAL` samples, or after an invalidation).
///
/// The returned clone owns one reference to each of its service handles;
/// wrap it in an [`EntriesGuard`] (as all callers do) to release them.
fn gpu_entries() -> Vec<GpuEntry> {
    let mut guard = GPU_CACHE.lock().unwrap();
    let due = match guard.as_mut() {
        None => true,
        Some(cache) => {
            cache.samples_since_refresh += 1;
            cache.samples_since_refresh >= GPU_REFRESH_INTERVAL
        }
    };
    if due {
        // lookup_gpu_entries() retains every handle it keeps; the old cache
        // still holds its own references, so release those now (superseded
        // or detached devices drop to zero and are IOObjectRelease'd).
        let found = lookup_gpu_entries();
        if let Some(old) = guard.as_ref() {
            release_entries(&old.entries);
        }
        if found.is_empty() {
            // Nothing found yet (or all detached) — do not cache the empty
            // result; retry on the next call.
            *guard = None;
        } else {
            *guard = Some(GpuCache {
                entries: found,
                samples_since_refresh: 0,
            });
        }
    }
    let clone = guard.as_ref().map(|c| c.entries.clone()).unwrap_or_default();
    retain_entries(&clone);
    clone
}

/// Enumerate every GPU accelerator service.
///
/// NOTE: the dictionary returned by `IOServiceMatching` must NOT be released
/// by us — `IOServiceGetMatchingServices` is declared `CF_RELEASES_ARGUMENT`
/// and always consumes one reference of the matching dictionary (observed
/// 2026-09-11: CFReleasing it afterwards segfaults).
///
/// Ownership: every handle returned by `IOIteratorNext` is immediately
/// retained in `SERVICE_REFS`. Kept services keep that reference (the cache
/// now owns it); services we skip — or that are duplicates across classes,
/// since a single device can match both `IOAccelerator` and
/// `AGXAccelerator` — are released right away.
fn lookup_gpu_entries() -> Vec<GpuEntry> {
    let mut entries = Vec::new();
    let mut seen: HashSet<u32> = HashSet::new();
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

        let mut service = unsafe { IOIteratorNext(iterator) };
        while service != 0 {
            // We now own this reference.
            retain_service(service);
            // A device can match multiple classes; keep only the first.
            if seen.insert(service) && read_device_utilization(service).is_some() {
                let name = read_string_property(service, "model")
                    .or_else(|| read_string_property(service, "name"))
                    .unwrap_or_else(|| "GPU".to_string());
                // Device-derived, position-independent id (the service
                // handle is stable for the device's lifetime).
                let id = format!("macos-gpu-{}", service);
                entries.push(GpuEntry {
                    service,
                    id,
                    name,
                });
            } else {
                // Skipped or duplicate — release the reference we just took.
                release_service(service);
            }
            service = unsafe { IOIteratorNext(iterator) };
        }
        // Release the iterator; kept services are referenced separately.
        unsafe { IOObjectRelease(iterator) };
    }
    entries
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
