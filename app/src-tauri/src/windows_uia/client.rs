use std::time::Duration;

#[cfg(target_os = "windows")]
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
#[cfg(target_os = "windows")]
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, UIA_E_ELEMENTNOTAVAILABLE,
};

#[derive(Debug, Clone)]
pub struct UiaClient {
    #[cfg(target_os = "windows")]
    automation: IUIAutomation,
}

impl UiaClient {
    #[cfg(target_os = "windows")]
    pub fn new() -> Result<Self, String> {
        unsafe {
            let automation: IUIAutomation =
                CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
                    .map_err(|err| format!("Failed to create UIAutomation client: {err}"))?;
            Ok(Self { automation })
        }
    }

    #[cfg(not(target_os = "windows"))]
    pub fn new() -> Result<Self, String> {
        Err("UI Automation is only supported on Windows".to_string())
    }

    #[cfg(target_os = "windows")]
    pub fn get_focused_element_with_retry(
        &self,
        retries: u32,
        delay_ms: u64,
    ) -> Result<IUIAutomationElement, String> {
        let mut attempt = 0u32;
        loop {
            attempt += 1;
            let result = unsafe { self.automation.GetFocusedElement() };
            match result {
                Ok(element) => return Ok(element),
                Err(err)
                    if err.code().0 as u32 == UIA_E_ELEMENTNOTAVAILABLE && attempt <= retries =>
                {
                    std::thread::sleep(Duration::from_millis(delay_ms));
                    continue;
                }
                Err(err) => {
                    return Err(format!("Failed to get focused element: {err}"));
                }
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    pub fn get_focused_element_with_retry(
        &self,
        _retries: u32,
        _delay_ms: u64,
    ) -> Result<(), String> {
        Err("UI Automation is only supported on Windows".to_string())
    }
}
