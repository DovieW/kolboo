#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{RPC_E_CHANGED_MODE, S_FALSE, S_OK};
#[cfg(target_os = "windows")]
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

/// Guard for COM initialization on the current thread (MTA).
#[derive(Debug)]
pub struct ComMtaGuard {
    initialized: bool,
}

impl ComMtaGuard {
    pub fn new(initialized: bool) -> Self {
        Self { initialized }
    }
}

impl Drop for ComMtaGuard {
    fn drop(&mut self) {
        #[cfg(target_os = "windows")]
        if self.initialized {
            unsafe {
                CoUninitialize();
            }
        }
    }
}

/// Initialize COM on the current thread in MTA mode.
///
/// Returns a guard that will call `CoUninitialize` on drop when appropriate.
pub fn initialize_com_mta() -> Result<ComMtaGuard, String> {
    #[cfg(target_os = "windows")]
    unsafe {
        let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
        if hr == S_OK || hr == S_FALSE {
            return Ok(ComMtaGuard::new(true));
        }

        if hr == RPC_E_CHANGED_MODE {
            // COM already initialized on this thread with a different apartment.
            // We cannot change it here; proceed without uninitializing.
            return Ok(ComMtaGuard::new(false));
        }

        Err(format!("Failed to initialize COM (MTA): {hr:?}"))
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(ComMtaGuard::new(false))
    }
}
