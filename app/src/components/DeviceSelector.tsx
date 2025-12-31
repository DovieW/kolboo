import { Loader, Select } from "@mantine/core";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import { useSettings, useUpdateSelectedMic } from "../lib/queries";

interface AudioDevice {
  deviceId: string;
  label: string;
  name: string;
}

interface BackendAudioInputDeviceInfo {
  id: string;
  name: string;
}

export function DeviceSelector() {
  const { data: settings, isLoading: settingsLoading } = useSettings();
  const updateSelectedMic = useUpdateSelectedMic();
  const [devices, setDevices] = useState<AudioDevice[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const migratedLegacyMicRef = useRef(false);

  const MIC_ID_PREFIX = "mic:v1:";

  useEffect(() => {
    async function loadDevices() {
      try {
        const [deviceInfos, defaultName] = await Promise.all([
          invoke<BackendAudioInputDeviceInfo[]>("list_audio_input_devices_v2"),
          invoke<string | null>("get_default_audio_input_device_name"),
        ]);

        // Mantine Select requires option values to be unique.
        // Be defensive even though the backend guarantees unique IDs.
        const byId = new Map<string, AudioDevice>();
        for (const d of deviceInfos ?? []) {
          const id = typeof d?.id === "string" ? d.id : "";
          const name = typeof d?.name === "string" ? d.name : "";
          if (!id || !name) continue;
          if (byId.has(id)) continue;

          const label =
            defaultName && name === defaultName ? `${name} (Default)` : name;

          byId.set(id, { deviceId: id, name, label });
        }

        setDevices(Array.from(byId.values()));
        setError(null);
      } catch (err) {
        setError("Could not list microphones from the backend.");
        console.error("Failed to list backend microphones:", err);
      } finally {
        setIsLoading(false);
      }
    }

    loadDevices();

    return;
  }, []);

  const handleChange = (value: string | null) => {
    // null or empty string means "default"
    const micId = value === "" || value === "default" ? null : value;
    updateSelectedMic.mutate(micId);
  };

  const selectData = [
    { value: "default", label: "System Default" },
    ...devices
      .filter((device) => device.deviceId !== "default")
      .map((device) => ({
        value: device.deviceId,
        label: device.label,
      })),
  ];

  const disabled = isLoading || settingsLoading || Boolean(error);
  const description = "Select which microphone to use for dictation";

  // If settings already point to a specific mic id, ensure it exists in the Select
  // options even before enumeration completes, so the control doesn't appear blank.
  const storedMicId = settings?.selected_mic_id ?? null;
  const selectedMicId = (() => {
    if (!storedMicId || storedMicId === "default") return "default";
    if (selectData.some((d) => d.value === storedMicId)) return storedMicId;

    // Backward compatibility: older builds stored the CPAL *name*.
    // If we find a matching device name, prefer its (unique) encoded ID.
    const legacyMatch = devices.find((d) => d.name === storedMicId);
    if (legacyMatch) return legacyMatch.deviceId;

    // Unknown/removed device; keep the stored value visible as a placeholder.
    return storedMicId;
  })();

  // One-time migration: if settings contain a legacy *name* and we can map it to a
  // unique backend ID, persist the new ID. This avoids ambiguous backend selection
  // when multiple devices share the same friendly name.
  useEffect(() => {
    if (migratedLegacyMicRef.current) return;
    if (!storedMicId) return;
    if (storedMicId === "default") return;
    if (storedMicId.startsWith(MIC_ID_PREFIX)) return;
    if (settingsLoading || isLoading) return;
    if (error) return;

    const legacyMatch = devices.find((d) => d.name === storedMicId);
    if (!legacyMatch) return;
    if (!legacyMatch.deviceId.startsWith(MIC_ID_PREFIX)) return;

    migratedLegacyMicRef.current = true;
    updateSelectedMic.mutate(legacyMatch.deviceId);
  }, [
    storedMicId,
    devices,
    settingsLoading,
    isLoading,
    error,
    updateSelectedMic,
    MIC_ID_PREFIX,
  ]);
  if (
    selectedMicId &&
    selectedMicId !== "default" &&
    !selectData.some((d) => d.value === selectedMicId)
  ) {
    selectData.splice(1, 0, {
      value: selectedMicId,
      label: "Selected microphone",
    });
  }

  return (
    <div className="settings-row">
      <div>
        <p className="settings-label">Microphone</p>
        <p
          className="settings-description"
          style={error ? { color: "#ef4444" } : undefined}
        >
          {error ?? description}
        </p>
      </div>
      <div style={{ minWidth: 240 }}>
        <Select
          data={selectData}
          value={selectedMicId}
          onChange={handleChange}
          allowDeselect={false}
          disabled={disabled}
          rightSection={
            isLoading || settingsLoading ? (
              <Loader size={14} color="orange" />
            ) : undefined
          }
          rightSectionPointerEvents="none"
          className="device-selector"
          withCheckIcon={false}
          styles={{
            input: {
              backgroundColor: "var(--bg-elevated)",
              borderColor: "var(--border-default)",
              color: "var(--text-primary)",
            },
          }}
        />
      </div>
    </div>
  );
}
