use super::client::UiaClient;
use super::com::initialize_com_mta;
use super::types::WindowsTextTargetSnapshot;
#[cfg(target_os = "windows")]
use crate::windows_apps::get_foreground_process_path;

#[cfg(target_os = "windows")]
use windows::Win32::System::Variant::{VARIANT, VT_BOOL, VT_I4, VT_UI4};
#[cfg(target_os = "windows")]
use windows::Win32::UI::Accessibility::{
    IUIAutomationElement, UIA_IsEnabledPropertyId, UIA_IsPasswordPropertyId,
    UIA_ValueIsReadOnlyPropertyId, UIA_PROPERTY_ID,
};

#[cfg(target_os = "windows")]
fn variant_to_bool(value: &VARIANT) -> Option<bool> {
    unsafe {
        let vt = value.Anonymous.Anonymous.vt.0 as u32;
        if vt == VT_BOOL.0 as u32 {
            return Some(value.Anonymous.Anonymous.Anonymous.boolVal.0 != 0);
        }
        if vt == VT_I4.0 as u32 {
            return Some(value.Anonymous.Anonymous.Anonymous.lVal != 0);
        }
        if vt == VT_UI4.0 as u32 {
            return Some(value.Anonymous.Anonymous.Anonymous.ulVal != 0);
        }
    }

    None
}

#[cfg(target_os = "windows")]
fn read_bool_property(
    element: &IUIAutomationElement,
    property_id: UIA_PROPERTY_ID,
) -> Option<bool> {
    let value = unsafe { element.GetCurrentPropertyValue(property_id) }.ok()?;
    variant_to_bool(&value)
}

#[cfg(target_os = "windows")]
pub fn capture_snapshot(
    element: &IUIAutomationElement,
    captured_at_ms: u64,
) -> Result<WindowsTextTargetSnapshot, String> {
    let is_password = read_bool_property(element, UIA_IsPasswordPropertyId);
    let is_enabled = read_bool_property(element, UIA_IsEnabledPropertyId);
    let is_read_only = read_bool_property(element, UIA_ValueIsReadOnlyPropertyId);

    log::debug!(
        "UIA snapshot: password={:?} enabled={:?} read_only={:?}",
        is_password,
        is_enabled,
        is_read_only
    );

    // Best-effort: process id is useful for app identity, but may fail in some controls.
    let process_id = unsafe { element.CurrentProcessId() }
        .ok()
        .map(|pid| pid as u32);
    let exe_path = get_foreground_process_path();

    Ok(WindowsTextTargetSnapshot {
        captured_at_ms,
        process_id,
        exe_path,
        window_title: None,
        uia_runtime_id: None,
        is_password,
        is_enabled,
        is_read_only,
        supports_text_pattern: false,
        supports_value_pattern: false,
    })
}

#[cfg(target_os = "windows")]
pub fn capture_focused_snapshot() -> Result<WindowsTextTargetSnapshot, String> {
    let _guard = initialize_com_mta()?;
    let client = UiaClient::new()?;
    let element = client.get_focused_element_with_retry(2, 40)?;
    let captured_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    capture_snapshot(&element, captured_at_ms)
}

#[cfg(not(target_os = "windows"))]
pub fn capture_snapshot(
    _element: &(),
    _captured_at_ms: u64,
) -> Result<WindowsTextTargetSnapshot, String> {
    Err("UI Automation is only supported on Windows".to_string())
}
