import { Button, Loader, Select } from "@mantine/core";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import { formatErrorMessage } from "../lib/formatError";
import { useSettings, useUpdateSelectedMic } from "../lib/queries";
import type { MicTestAudioLevelPayload } from "../lib/tauri";

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
	const [micTestError, setMicTestError] = useState<string | null>(null);
	const [isMicTesting, setIsMicTesting] = useState(false);
	const [micPeak, setMicPeak] = useState(0);
	const micTestDeviceIdRef = useRef<string | null>(null);
	const micTestSessionIdRef = useRef<number | null>(null);
	const micTestStartInFlightRef = useRef(false);
	const prevSelectedMicIdRef = useRef<string | null>(null);
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

	// Realtime mic test meter events (backend CPAL stream).
	useEffect(() => {
		let unlisten: (() => void) | null = null;

		const setup = async () => {
			try {
				unlisten = await listen<MicTestAudioLevelPayload>(
					"mic-test-audio-level",
					(event) => {
						const p = event.payload;
						if (!p) return;

						const sid =
							typeof p.session_id === "number" && Number.isFinite(p.session_id)
								? p.session_id
								: null;
						const currentSid = micTestSessionIdRef.current;

						// Ignore stale events from previous sessions.
						if (sid != null && currentSid != null && sid !== currentSid) {
							return;
						}

						if (typeof p.active === "boolean") {
							setIsMicTesting(p.active);
							if (!p.active) {
								setMicPeak(0);
								micTestDeviceIdRef.current = null;
								micTestSessionIdRef.current = null;
							}

							// When a session becomes active, lock onto its session id.
							if (p.active && sid != null) {
								micTestSessionIdRef.current = sid;
							}
						}

						const peak =
							typeof p.peak === "number" && Number.isFinite(p.peak)
								? Math.max(0, Math.min(1, p.peak))
								: 0;

						// Peak-hold-ish UI: rise quickly, decay slowly.
						setMicPeak((prev) => Math.max(peak, prev * 0.82));
					},
				);
			} catch (e) {
				// If the listener can't be installed (should be rare), keep UI usable.
				console.warn("Failed to listen to mic-test-audio-level:", e);
			}
		};

		setup();

		return () => {
			try {
				unlisten?.();
			} catch {
				// ignore
			}
		};
	}, []);

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
	]);

	// If the selected mic changes while the user is testing, force-stop the test.
	// This avoids rapid start/stop loops and makes switching devices predictable.
	useEffect(() => {
		const prev = prevSelectedMicIdRef.current;
		prevSelectedMicIdRef.current = selectedMicId;

		if (!isMicTesting) return;
		if (!prev) return;
		if (prev === selectedMicId) return;
		if (micTestStartInFlightRef.current) return;

		// If the mic was switched, stop the current test session.
		setMicTestError(null);
		invoke<void>("mic_test_stop_meter")
			.catch((e) => {
				console.warn("Failed to stop mic test meter after mic switch:", e);
			})
			.finally(() => {
				setIsMicTesting(false);
				setMicPeak(0);
				micTestDeviceIdRef.current = null;
				micTestSessionIdRef.current = null;
			});
	}, [isMicTesting, selectedMicId]);

	// Ensure we stop the backend mic test stream when the settings row unmounts.
	useEffect(() => {
		return () => {
			invoke<void>("mic_test_stop_meter").catch(() => {
				// ignore
			});
		};
	}, []);

	const toggleMicTest = async () => {
		setMicTestError(null);

		if (isMicTesting) {
			try {
				await invoke<void>("mic_test_stop_meter");
			} catch (e) {
				console.warn("Failed to stop mic test meter:", e);
			} finally {
				setIsMicTesting(false);
				setMicPeak(0);
				micTestDeviceIdRef.current = null;
				micTestSessionIdRef.current = null;
			}
			return;
		}

		const inputDeviceId =
			!selectedMicId || selectedMicId === "default" ? null : selectedMicId;

		try {
			// Optimistically set so the restart effect doesn't immediately fire if
			// the backend event arrives before this await resolves.
			micTestStartInFlightRef.current = true;
			micTestDeviceIdRef.current = inputDeviceId;
			setIsMicTesting(true);

			await invoke<void>("mic_test_start_meter", { args: { inputDeviceId } });
			// Session id will be populated from the first backend event.
		} catch (e) {
			console.warn("Failed to start mic test meter:", e);
			setMicTestError(formatErrorMessage(e));
			setIsMicTesting(false);
			setMicPeak(0);
			micTestDeviceIdRef.current = null;
			micTestSessionIdRef.current = null;
		} finally {
			micTestStartInFlightRef.current = false;
		}
	};

	const meterLevel = (() => {
		// Map amplitude to a more perceptual dBFS meter.
		// Typical speaking often lands around -30..-15 dBFS; this range makes it visible.
		const peak = Math.max(0, Math.min(1, micPeak));
		const dbfs = 20 * Math.log10(Math.max(peak, 1e-4));

		// Calibrate UI feel: -55dBFS => 0%, -5dBFS => 100%
		const minDb = -55;
		const maxDb = -5;
		const t = (dbfs - minDb) / (maxDb - minDb);

		// Slight curve so low-mid levels are easier to see.
		const curved = Math.max(0, Math.min(1, t)) ** 0.75;
		return Math.max(0, Math.min(1, curved));
	})();

	const meterColor = (() => {
		const peak = Math.max(0, Math.min(1, micPeak));
		const dbfs = 20 * Math.log10(Math.max(peak, 1e-4));

		// Keep red only for *very* hot signals (near clipping), yellow for loud.
		if (dbfs >= -3) return "#ef4444"; // red-500
		if (dbfs >= -12) return "#f59e0b"; // amber-500
		return "#22c55e"; // green-500
	})();
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
					{error ?? micTestError ?? description}
				</p>
			</div>
			<div
				className="settings-row-actions"
				style={{ flexWrap: "wrap", justifyContent: "flex-end", flexShrink: 1 }}
			>
				<div
					className={
						isMicTesting
							? "mic-test-meter mic-test-meter--active"
							: "mic-test-meter"
					}
					role="progressbar"
					aria-label="Microphone level"
					aria-valuemin={0}
					aria-valuemax={100}
					aria-valuenow={Math.round(meterLevel * 100)}
					title={
						isMicTesting
							? "Speak into the mic to see the level"
							: "Click Test to show mic level"
					}
				>
					<div
						className="mic-test-meter-fill"
						style={{
							width: `${Math.round(meterLevel * 100)}%`,
							backgroundColor: meterColor,
						}}
					/>
				</div>

				<Button
					color="gray"
					variant="default"
					onClick={() => void toggleMicTest()}
					disabled={disabled}
					styles={{
						root: {
							backgroundColor: "var(--bg-elevated)",
							borderColor: "var(--border-default)",
							color: "var(--text-primary)",
							height: 36,
						},
					}}
				>
					{isMicTesting ? "Stop" : "Test"}
				</Button>

				<div style={{ width: 420, maxWidth: "100%" }}>
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
		</div>
	);
}
