use crate::platform::GpuMonitor;
use std::io;
use std::sync::Mutex;
use windows::core::PCWSTR;
use windows::Win32::System::Performance::{
    PDH_FMT_COUNTERVALUE, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY,
    PDH_MORE_DATA, PdhAddEnglishCounterW, PdhCollectQueryData, PdhCloseQuery,
    PdhGetFormattedCounterArrayW, PdhGetFormattedCounterValue, PdhOpenQueryW,
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
/// instance per GPU engine (3D, Copy, Video Decode, ...); we report the max
/// across engines as the overall GPU utilization, mirroring how Task
/// Manager surfaces "GPU busy".
const GPU_UTIL_COUNTER: PCWSTR =
    windows::core::w!("\\GPU Engine(*)\\Utilization Percentage");

impl GpuMonitor for WindowsGpuMonitor {
    fn get_gpu_usage() -> io::Result<f64> {
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
            let ret = unsafe {
                PdhAddEnglishCounterW(hquery, GPU_UTIL_COUNTER, 0, &mut hcounter)
            };
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

        // Wildcard counters expand into one instance per engine; the array
        // API returns them all in one call (two-pass buffer query).
        let mut size: u32 = 0;
        let mut count: u32 = 0;
        let ret = unsafe {
            PdhGetFormattedCounterArrayW(state.hcounter, PDH_FMT_DOUBLE, &mut size, &mut count, None)
        };
        if ret == PDH_MORE_DATA && count > 0 {
            let item_size = std::mem::size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>();
            let item_count = (size as usize) / item_size;
            let mut items: Vec<PDH_FMT_COUNTERVALUE_ITEM_W> = (0..item_count)
                .map(|_| PDH_FMT_COUNTERVALUE_ITEM_W::default())
                .collect();
            let ret = unsafe {
                PdhGetFormattedCounterArrayW(
                    state.hcounter,
                    PDH_FMT_DOUBLE,
                    &mut size,
                    &mut count,
                    Some(items.as_mut_ptr()),
                )
            };
            if ret == 0 {
                let mut max: Option<f64> = None;
                for item in items.iter() {
                    // PDH_CSTATUS_SUCCESS == 0; skip engines with no data.
                    if item.FmtValue.CStatus == 0 {
                        let v = unsafe { item.FmtValue.Anonymous.doubleValue };
                        max = Some(max.map_or(v, |m| m.max(v)));
                    }
                }
                return max
                    .ok_or_else(|| io::Error::other("No GPU engine utilization data available"));
            }
            Err(io::Error::other(format!(
                "PdhGetFormattedCounterArrayW failed: 0x{:08X}",
                ret
            )))
        } else if ret == 0 {
            // Single-instance counter (no wildcard expansion).
            let mut status: u32 = 0;
            let mut value = PDH_FMT_COUNTERVALUE::default();
            let ret = unsafe {
                PdhGetFormattedCounterValue(
                    state.hcounter,
                    PDH_FMT_DOUBLE,
                    Some(&mut status),
                    &mut value,
                )
            };
            if ret == 0 && status == 0 {
                return Ok(unsafe { value.Anonymous.doubleValue });
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
}
