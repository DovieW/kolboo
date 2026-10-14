//! Audio input-device selection helpers.
//!
//! CPAL device descriptions are user-facing strings rather than stable OS ids.
//! This module keeps the session-stable selection token logic in one place so
//! UI options, persisted selections, and runtime fallback behavior cannot drift.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use cpal::traits::{DeviceTrait, HostTrait};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
#[cfg(not(target_os = "linux"))]
use std::collections::HashMap;
use std::collections::HashSet;

pub(super) enum InputDevice {
    Cpal(cpal::Device),
    #[cfg(target_os = "linux")]
    Pulse(String),
}

impl InputDevice {
    pub(super) fn default_input_config(
        &self,
    ) -> Result<cpal::SupportedStreamConfig, cpal::DefaultStreamConfigError> {
        match self {
            Self::Cpal(device) => device.default_input_config(),
            #[cfg(target_os = "linux")]
            Self::Pulse(_) => Ok(cpal::SupportedStreamConfig::new(
                1,
                48000,
                cpal::SupportedBufferSize::Unknown,
                cpal::SampleFormat::F32,
            )),
        }
    }
}

const MIC_DEVICE_ID_PREFIX: &str = "mic:v1:";
#[cfg(target_os = "linux")]
const ALSA_ID_PREFIX: &str = "mic:alsa:v1:";

/// Public device descriptor for the frontend.
///
/// NOTE: `id` is a stable-ish *selection token* for this session, not a true OS device ID.
/// It is guaranteed unique within the returned list.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AudioInputDeviceInfo {
    pub id: String,
    pub name: String,
}

#[cfg(any(not(target_os = "linux"), test))]
fn encode_mic_device_id(name: &str, ordinal_for_name: usize) -> String {
    // Base64url without padding so the token remains URL/UI friendly.
    let name_b64 = URL_SAFE_NO_PAD.encode(name.as_bytes());
    format!("{MIC_DEVICE_ID_PREFIX}{name_b64}:{ordinal_for_name}")
}

fn decode_mic_device_id(id: &str) -> Option<(String, usize)> {
    // Format: mic:v1:<base64url(name)>:<ordinal>
    let rest = id.strip_prefix(MIC_DEVICE_ID_PREFIX)?;
    let (name_b64, ordinal_str) = rest.rsplit_once(':')?;
    let ordinal = ordinal_str.parse::<usize>().ok()?;
    let name_bytes = URL_SAFE_NO_PAD.decode(name_b64).ok()?;
    let name = String::from_utf8(name_bytes).ok()?;
    Some((name, ordinal))
}

fn normalize_input_device_selection(input_device: Option<&str>) -> Option<(String, usize, bool)> {
    // Returns (desired_name, desired_ordinal, is_encoded_id). Keeping the
    // boolean explicit is important: encoded IDs must not use legacy contains()
    // fallback because that can pick the wrong duplicate-name microphone.
    let raw = input_device
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != "default")?;

    if let Some((name, ordinal)) = decode_mic_device_id(raw) {
        return Some((name, ordinal, true));
    }

    Some((raw.to_string(), 0, false))
}

pub(crate) fn select_input_device_index_from_names(
    names: &[String],
    selection: Option<&str>,
) -> Option<usize> {
    let (desired_name, desired_ordinal, is_encoded) = normalize_input_device_selection(selection)?;

    // Prefer exact-name matching with ordinal disambiguation.
    let mut ordinal_for_name: usize = 0;
    for (idx, name) in names.iter().enumerate() {
        if name == &desired_name {
            if ordinal_for_name == desired_ordinal {
                return Some(idx);
            }
            ordinal_for_name = ordinal_for_name.saturating_add(1);
        }
    }

    // Legacy fallback: some older stored values used partial matches.
    // For encoded IDs, do NOT do a contains() fallback (could pick the wrong device).
    if is_encoded {
        return None;
    }

    for (idx, name) in names.iter().enumerate() {
        if name.contains(&desired_name) {
            return Some(idx);
        }
    }

    None
}

pub(super) fn select_input_device_from_host(
    host: &cpal::Host,
    selection: Option<&str>,
) -> Result<InputDevice, super::AudioCaptureError> {
    #[cfg(target_os = "linux")]
    if let Some(id) = selection.filter(|id| id.starts_with(super::linux_input::ID_PREFIX)) {
        return super::linux_input::source_name(id)
            .map(InputDevice::Pulse)
            .ok_or(super::AudioCaptureError::NoInputDevice);
    }
    #[cfg(target_os = "linux")]
    if let Some(id) = selection.and_then(|id| id.strip_prefix(ALSA_ID_PREFIX)) {
        let id = URL_SAFE_NO_PAD
            .decode(id)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .and_then(|id| id.parse::<cpal::DeviceId>().ok());
        return id
            .and_then(|id| host.device_by_id(&id))
            .map(InputDevice::Cpal)
            .ok_or(super::AudioCaptureError::NoInputDevice);
    }
    select_legacy_input_device_from_host(host, selection)
        .or_else(|| host.default_input_device())
        .map(InputDevice::Cpal)
        .ok_or(super::AudioCaptureError::NoInputDevice)
}

fn select_legacy_input_device_from_host(
    host: &cpal::Host,
    selection: Option<&str>,
) -> Option<cpal::Device> {
    let Ok(devices) = host.input_devices() else {
        return None;
    };

    let mut device_list: Vec<(cpal::Device, String)> = Vec::new();
    for d in devices {
        let Ok(desc) = d.description() else { continue };
        let name = desc.to_string();
        device_list.push((d, name));
    }

    let names: Vec<String> = device_list.iter().map(|(_, name)| name.clone()).collect();
    let idx = select_input_device_index_from_names(&names, selection)?;

    device_list.into_iter().nth(idx).map(|(device, _)| device)
}

/// Get the list of available input devices.
#[cfg_attr(not(test), allow(dead_code))]
pub fn list_input_devices() -> Vec<String> {
    let host = cpal::default_host();
    host.input_devices()
        .map(|devices| {
            // Defensive: CPAL device descriptions are not guaranteed unique on Windows.
            // The legacy API returns names only; dedupe to avoid downstream UI crashes
            // in case a caller uses names as unique keys.
            let mut out: Vec<String> = Vec::new();
            let mut seen: HashSet<String> = HashSet::new();

            for name in devices.filter_map(|d| d.description().ok().map(|desc| desc.to_string())) {
                if seen.insert(name.clone()) {
                    out.push(name);
                }
            }

            out
        })
        .unwrap_or_default()
}

/// Get the list of available input devices, with unique IDs suitable for UI option values.
///
/// Linux uses sound-server source IDs or canonical ALSA route IDs; other
/// platforms retain legacy `mic:v1:<base64url(name)>:<ordinal>` selections.
pub fn list_input_devices_v2() -> Vec<AudioInputDeviceInfo> {
    #[cfg(target_os = "linux")]
    let server_devices = super::linux_input::list_devices();
    #[cfg(not(target_os = "linux"))]
    let server_devices = None;
    input_device_options(&cpal::default_host(), server_devices)
}

pub(super) fn input_device_options(
    host: &cpal::Host,
    server_devices: Option<Vec<AudioInputDeviceInfo>>,
) -> Vec<AudioInputDeviceInfo> {
    if let Some(devices) = server_devices {
        return devices;
    }
    let Ok(devices) = host.input_devices() else {
        return Vec::new();
    };

    #[cfg(target_os = "linux")]
    {
        alsa_fallback_options(
            devices
                .filter_map(|device| {
                    Some((
                        device.id().ok()?.to_string(),
                        device.description().ok()?.to_string(),
                    ))
                })
                .collect(),
        )
    }

    #[cfg(not(target_os = "linux"))]
    {
        let mut name_ordinals: HashMap<String, usize> = HashMap::new();
        let mut out: Vec<AudioInputDeviceInfo> = Vec::new();

        for d in devices {
            let Ok(desc) = d.description() else { continue };
            let name = desc.to_string();
            let ordinal = name_ordinals.get(&name).copied().unwrap_or(0);
            name_ordinals.insert(name.clone(), ordinal.saturating_add(1));

            out.push(AudioInputDeviceInfo {
                id: encode_mic_device_id(&name, ordinal),
                name,
            });
        }

        // Extra defensive: ensure uniqueness even if encoding logic changes.
        // (Should never trigger, but guarantees the UI can't crash.)
        let mut seen_ids: HashMap<String, usize> = HashMap::new();
        for device in &mut out {
            let n = seen_ids.get(&device.id).copied().unwrap_or(0);
            if n > 0 {
                device.id = format!("{}:dup{}", device.id, n);
            }
            seen_ids.insert(device.id.clone(), n.saturating_add(1));
        }

        out
    }
}

#[cfg(target_os = "linux")]
fn alsa_fallback_options(devices: Vec<(String, String)>) -> Vec<AudioInputDeviceInfo> {
    let mut seen = HashSet::new();
    devices
        .into_iter()
        .filter_map(|(id, name)| {
            let (_, pcm) = id.split_once(':')?;
            let route = pcm.split(':').next()?;
            // CPAL enumerates hw + plughw + hints for the same endpoints. Keep the
            // converting physical route and intentional user-defined PCM names.
            if matches!(
                route,
                "null"
                    | "default"
                    | "sysdefault"
                    | "pulse"
                    | "pipewire"
                    | "hw"
                    | "front"
                    | "rear"
                    | "center_lfe"
                    | "side"
                    | "surround21"
                    | "surround40"
                    | "surround41"
                    | "surround50"
                    | "surround51"
                    | "surround71"
                    | "iec958"
                    | "hdmi"
                    | "dmix"
                    | "dsnoop"
                    | "usbstream"
            ) || !seen.insert(id.clone())
            {
                return None;
            }
            Some(AudioInputDeviceInfo {
                id: format!("{ALSA_ID_PREFIX}{}", URL_SAFE_NO_PAD.encode(id.as_bytes())),
                name: format!("{name} · {pcm}"),
            })
        })
        .collect()
}

/// Get information about the default input device.
#[cfg_attr(not(test), allow(dead_code))]
pub fn get_default_input_device_info() -> Option<(String, u32, u16)> {
    let host = cpal::default_host();
    let device = host.default_input_device()?;
    let name = device.description().ok()?.to_string();
    let config = device.default_input_config().ok()?;
    Some((name, config.sample_rate(), config.channels()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn alsa_only_fallback_excludes_aliases_without_merging_distinct_ports() {
        let candidates = [
            ("alsa:null", "Discard"),
            ("alsa:default", "Default"),
            ("alsa:pipewire", "PipeWire"),
            ("alsa:pulse", "Pulse"),
            ("alsa:hw:CARD=1,DEV=0", "Built-in"),
            ("alsa:dsnoop:CARD=1,DEV=0", "Built-in"),
            ("alsa:plughw:CARD=1,DEV=0", "Built-in"),
            ("alsa:plughw:CARD=1,DEV=6", "Built-in"),
            ("alsa:plughw:CARD=2,DEV=0", "USB"),
            ("alsa:plughw:CARD=2,DEV=0", "Duplicate"),
            ("alsa:my_custom_microphone", "Virtual microphone"),
            ("not-an-id", "Malformed"),
        ];
        let devices = alsa_fallback_options(
            candidates
                .into_iter()
                .map(|(id, name)| (id.into(), name.into()))
                .collect(),
        );
        assert_eq!(devices.len(), 4);
        assert_ne!(devices[0].id, devices[1].id);
        assert!(devices[3].name.contains("Virtual microphone"));
        assert_eq!(
            String::from_utf8(
                URL_SAFE_NO_PAD
                    .decode(devices[0].id.strip_prefix(ALSA_ID_PREFIX).unwrap())
                    .unwrap()
            )
            .unwrap(),
            "alsa:plughw:CARD=1,DEV=0"
        );
    }

    #[test]
    fn mic_device_id_roundtrips_duplicate_ordinals() {
        let name = "Built-in Mic";
        let id = encode_mic_device_id(name, 2);
        let decoded = decode_mic_device_id(&id).expect("Expected valid mic device id");
        assert_eq!(decoded, (name.to_string(), 2));
    }

    #[test]
    fn normalizes_default_blank_legacy_and_encoded_selections() {
        assert_eq!(normalize_input_device_selection(None), None);
        assert_eq!(normalize_input_device_selection(Some("")), None);
        assert_eq!(normalize_input_device_selection(Some("default")), None);

        let encoded = encode_mic_device_id("USB Mic", 1);
        assert_eq!(
            normalize_input_device_selection(Some(&encoded)),
            Some(("USB Mic".to_string(), 1, true))
        );

        assert_eq!(
            normalize_input_device_selection(Some("Plantronics")),
            Some(("Plantronics".to_string(), 0, false))
        );
    }

    #[test]
    fn selected_device_prefers_exact_ordinal_then_legacy_contains() {
        let names = vec!["Mic A", "Mic B", "Mic A"]
            .into_iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();

        assert_eq!(
            select_input_device_index_from_names(&names, Some("Mic A")),
            Some(0)
        );

        let encoded = encode_mic_device_id("Mic A", 1);
        assert_eq!(
            select_input_device_index_from_names(&names, Some(&encoded)),
            Some(2)
        );

        assert_eq!(
            select_input_device_index_from_names(&names, Some("Mic")),
            Some(0)
        );

        let missing = encode_mic_device_id("Unknown", 0);
        assert_eq!(
            select_input_device_index_from_names(&names, Some(&missing)),
            None
        );
    }
}
