//! Tray controls share the backend's microphone selection tokens and settings writes.
use crate::audio_capture::AudioInputDeviceInfo;
use crate::pipeline::{PipelineState, SharedPipeline};
use tauri::{
    menu::{CheckMenuItemBuilder, MenuItem, MenuItemBuilder, Submenu},
    AppHandle, Manager,
};

pub(super) const MIC_PREFIX: &str = "tray-mic::";
pub(super) const REFRESH_ID: &str = "tray-refresh";

#[derive(Debug, PartialEq)]
struct MicOption {
    id: String,
    label: String,
    selected: bool,
    enabled: bool,
}

fn microphone_options(
    devices: &[AudioInputDeviceInfo],
    selected: Option<&str>,
    idle: bool,
) -> Vec<MicOption> {
    let selected = selected
        .map(str::trim)
        .filter(|id| !id.trim().is_empty())
        .unwrap_or("default");
    let names = devices
        .iter()
        .map(|device| device.name.clone())
        .collect::<Vec<_>>();
    let chosen = devices
        .iter()
        .position(|device| device.id == selected)
        .or_else(|| {
            crate::audio_capture::select_input_device_index_from_names(&names, Some(selected))
        });
    let mut options = vec![MicOption {
        id: "default".into(),
        label: "System default".into(),
        selected: selected == "default",
        enabled: idle,
    }];
    for (index, device) in devices.iter().enumerate() {
        let count = devices
            .iter()
            .filter(|other| other.name == device.name)
            .count();
        let ordinal = devices[..index]
            .iter()
            .filter(|other| other.name == device.name)
            .count()
            + 1;
        let label = if count > 1 {
            format!("{} · Device {ordinal} of {count}", device.name)
        } else {
            device.name.clone()
        };
        options.push(MicOption {
            id: device.id.clone(),
            label,
            selected: chosen == Some(index),
            enabled: idle,
        });
    }
    if !options.iter().any(|option| option.selected) {
        options.push(MicOption {
            id: selected.into(),
            label: "Selected microphone unavailable".into(),
            selected: true,
            enabled: false,
        });
    }
    options
}

fn idle(app: &AppHandle) -> bool {
    let meter_idle = app
        .try_state::<crate::state::MicTestMeterState>()
        .is_none_or(|state| {
            state.inner.try_lock().is_ok_and(|inner| {
                inner
                    .cancel
                    .as_ref()
                    .is_none_or(|cancel| cancel.is_cancelled())
            })
        });
    meter_idle
        && app.try_state::<SharedPipeline>().is_some_and(|pipeline| {
            matches!(
                pipeline.try_state(),
                Some(PipelineState::Idle | PipelineState::Error)
            )
        })
}

pub(super) fn on_menu_event(app: &AppHandle, event: tauri::menu::MenuEvent) {
    match event.id.as_ref() {
        REFRESH_ID => super::refresh_tray_menu(app, super::build_tray_menu),
        id if id.starts_with(MIC_PREFIX) => {
            let app = app.clone();
            let id = id[MIC_PREFIX.len()..].to_string();
            tauri::async_runtime::spawn(async move {
                if let Err(message) = select_microphone(&app, &id).await {
                    crate::app_shared::emit_system_event(&app, "warning", &message, None);
                }
                super::refresh_tray_menu(&app, super::build_tray_menu);
            });
        }
        _ => {}
    }
}

pub(super) fn build(
    app: &AppHandle,
    devices: &[AudioInputDeviceInfo],
) -> tauri::Result<Submenu<tauri::Wry>> {
    let selected: Option<String> = crate::get_setting_from_store(app, "selected_mic_id", None);
    let menu = Submenu::with_id(app, "tray-microphones", "Microphone", true)?;
    for option in microphone_options(devices, selected.as_deref(), idle(app)) {
        let item = CheckMenuItemBuilder::new(option.label)
            .id(format!("{MIC_PREFIX}{}", option.id))
            .enabled(option.enabled)
            .checked(option.selected)
            .build(app)?;
        menu.append(&item)?;
    }
    let refresh = MenuItemBuilder::new("Refresh microphones")
        .id(REFRESH_ID)
        .build(app)?;
    menu.append(&refresh)?;
    Ok(menu)
}

pub(super) fn context_items(app: &AppHandle) -> tauri::Result<[MenuItem<tauri::Wry>; 2]> {
    let context = app
        .try_state::<crate::request_log::RequestLogStore>()
        .and_then(|store| store.latest_profile_preset());
    let (profile, preset) = context.unwrap_or_default();
    let profile = MenuItemBuilder::new(format!(
        "Last profile: {}",
        profile.as_deref().unwrap_or("Not used yet")
    ))
    .id("tray-last-profile")
    .enabled(false)
    .build(app)?;
    let preset = MenuItemBuilder::new(format!(
        "Last preset: {}",
        preset.as_deref().unwrap_or("None")
    ))
    .id("tray-last-preset")
    .enabled(false)
    .build(app)?;
    Ok([profile, preset])
}

pub(super) async fn select_microphone(app: &AppHandle, id: &str) -> Result<(), String> {
    apply_microphone_selection(app, id, &crate::audio_capture::list_input_devices_v2()).await
}

// Native enumeration is a snapshot; revalidate that snapshot at the persistence boundary.
// Keeping this transaction separate also permits deterministic device tests without hardware.
async fn apply_microphone_selection(
    app: &AppHandle,
    id: &str,
    devices: &[AudioInputDeviceInfo],
) -> Result<(), String> {
    // Revalidate a possibly stale menu. Active capture owns its device snapshot.
    if !idle(app) {
        return Err("Finish the current recording before changing microphones".into());
    }
    if id != "default" && !devices.iter().any(|device| device.id == id) {
        return Err("Microphone is no longer available; refresh the tray menu".into());
    }
    crate::commands::settings::settings_apply_patch(
        app.clone(),
        serde_json::Map::from_iter([(
            "selected_mic_id".into(),
            if id == "default" {
                serde_json::Value::Null
            } else {
                serde_json::Value::String(id.into())
            },
        )]),
        vec![],
    )
    .await
    .map_err(|_| "Microphone settings could not be saved")?;
    crate::commands::config::sync_pipeline_config(app.clone())
        .map_err(|_| "Microphone runtime could not be updated".to_string())
}

#[cfg(test)]
pub(crate) mod tests;
