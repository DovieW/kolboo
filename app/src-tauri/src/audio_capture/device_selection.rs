//! Audio input-device selection helpers.
//!
//! CPAL device descriptions are user-facing strings rather than stable OS ids.
//! This module keeps the session-stable selection token logic in one place so
//! UI options, persisted selections, and runtime fallback behavior cannot drift.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use cpal::traits::{DeviceTrait, HostTrait};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

const MIC_DEVICE_ID_PREFIX: &str = "mic:v1:";

/// Public device descriptor for the frontend.
///
/// NOTE: `id` is a stable-ish *selection token* for this session, not a true OS device ID.
/// It is guaranteed unique within the returned list.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AudioInputDeviceInfo {
    pub id: String,
    pub name: String,
}

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

fn select_input_device_index_from_names(
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
/// The ID format is `mic:v1:<base64url(name)>:<ordinal>` where ordinal is the 0-based
/// occurrence index for that exact name in the CPAL enumeration order.
pub fn list_input_devices_v2() -> Vec<AudioInputDeviceInfo> {
    let host = cpal::default_host();

    let Ok(devices) = host.input_devices() else {
        return Vec::new();
    };

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
