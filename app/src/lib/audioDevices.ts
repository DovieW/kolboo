import { formatErrorMessage } from "./formatError";
import type { AudioInputDeviceInfo } from "./tauri";

export interface AudioInputDevicesQueryData {
	devices: AudioInputDeviceInfo[];
	defaultDeviceName: string | null;
}

export interface MicSelectOption {
	value: string;
	label: string;
	name: string;
	duplicateIndex: number;
	duplicateCount: number;
}

export interface MissingSelectedMic {
	value: string;
	label: string;
	name: string;
}

export interface MicSelectorModel {
	defaultDeviceName: string | null;
	defaultOptionLabel: string;
	deviceOptions: MicSelectOption[];
	selectData: Array<{ value: string; label: string }>;
	selectedValue: string;
	selectedLabel: string;
	selectedSummaryLabel: string;
	selectedDevice: MicSelectOption | null;
	missingSelected: MissingSelectedMic | null;
	hasDevices: boolean;
	hasAnyDetectedInput: boolean;
	legacySelectionTargetId: string | null;
}

const MIC_DEVICE_ID_PREFIX = "mic:v1:";

function normalizeDeviceName(name: string | null | undefined): string | null {
	const trimmed = typeof name === "string" ? name.trim() : "";
	return trimmed.length > 0 ? trimmed : null;
}

function decodeBase64UrlBytes(value: string): Uint8Array | null {
	const normalized = value.replace(/-/g, "+").replace(/_/g, "/");
	const padded = normalized.padEnd(Math.ceil(normalized.length / 4) * 4, "=");

	if (typeof globalThis.atob === "function") {
		try {
			const binary = globalThis.atob(padded);
			return Uint8Array.from(binary, (char) => char.charCodeAt(0));
		} catch {
			return null;
		}
	}

	const bufferCtor = (globalThis as { Buffer?: typeof Buffer }).Buffer;
	if (!bufferCtor) return null;

	try {
		return Uint8Array.from(bufferCtor.from(padded, "base64"));
	} catch {
		return null;
	}
}

export function decodeMicDeviceIdName(
	id: string | null | undefined,
): string | null {
	if (typeof id !== "string" || !id.startsWith(MIC_DEVICE_ID_PREFIX)) {
		return null;
	}

	const rest = id.slice(MIC_DEVICE_ID_PREFIX.length);
	const splitIndex = rest.lastIndexOf(":");
	if (splitIndex <= 0) return null;

	const encodedName = rest.slice(0, splitIndex);
	const bytes = decodeBase64UrlBytes(encodedName);
	if (!bytes) return null;

	try {
		return normalizeDeviceName(new TextDecoder().decode(bytes));
	} catch {
		return null;
	}
}

function formatMicOptionLabel(
	name: string,
	duplicateIndex: number,
	duplicateCount: number,
): string {
	if (duplicateCount <= 1) return name;
	return `${name} · Device ${duplicateIndex + 1} of ${duplicateCount}`;
}

export function buildMicSelectorModel(params: {
	devices: AudioInputDeviceInfo[] | null | undefined;
	defaultDeviceName: string | null | undefined;
	storedMicId: string | null | undefined;
}): MicSelectorModel {
	const defaultDeviceName = normalizeDeviceName(params.defaultDeviceName);
	const storedMicId = normalizeDeviceName(params.storedMicId) ?? "default";

	const sanitizedDevices = (params.devices ?? [])
		.filter(
			(device) =>
				typeof device?.id === "string" &&
				device.id.trim().length > 0 &&
				typeof device?.name === "string" &&
				device.name.trim().length > 0,
		)
		.map((device) => ({
			id: device.id.trim(),
			name: device.name.trim(),
		}));

	const countsByName = new Map<string, number>();
	for (const device of sanitizedDevices) {
		countsByName.set(device.name, (countsByName.get(device.name) ?? 0) + 1);
	}

	const seenByName = new Map<string, number>();
	const deviceOptions = sanitizedDevices.map((device) => {
		const duplicateIndex = seenByName.get(device.name) ?? 0;
		seenByName.set(device.name, duplicateIndex + 1);

		const duplicateCount = countsByName.get(device.name) ?? 1;
		return {
			value: device.id,
			label: formatMicOptionLabel(device.name, duplicateIndex, duplicateCount),
			name: device.name,
			duplicateIndex,
			duplicateCount,
		};
	});

	const defaultOptionLabel = defaultDeviceName
		? `System Default: ${defaultDeviceName}`
		: "System Default — no default detected";

	const selectData: Array<{ value: string; label: string }> = [
		{ value: "default", label: defaultOptionLabel },
		...deviceOptions.map((option) => ({
			value: option.value,
			label: option.label,
		})),
	];

	let selectedValue = "default";
	let selectedLabel = defaultOptionLabel;
	let selectedSummaryLabel = defaultDeviceName
		? `System Default (${defaultDeviceName})`
		: "System Default";
	let selectedDevice: MicSelectOption | null = null;
	let missingSelected: MissingSelectedMic | null = null;
	let legacySelectionTargetId: string | null = null;

	if (storedMicId !== "default") {
		selectedDevice =
			deviceOptions.find((option) => option.value === storedMicId) ?? null;

		if (!selectedDevice && !storedMicId.startsWith(MIC_DEVICE_ID_PREFIX)) {
			// Older builds stored the device *name* only. Keep auto-migrating that
			// forward so duplicate-name microphones don't stay ambiguous forever.
			selectedDevice =
				deviceOptions.find((option) => option.name === storedMicId) ?? null;
			legacySelectionTargetId = selectedDevice?.value ?? null;
		}

		if (selectedDevice) {
			selectedValue = selectedDevice.value;
			selectedLabel = selectedDevice.label;
			selectedSummaryLabel = selectedDevice.label;
		} else {
			const missingName = decodeMicDeviceIdName(storedMicId) ?? storedMicId;
			missingSelected = {
				value: storedMicId,
				label: `Missing microphone: ${missingName}`,
				name: missingName,
			};
			selectedValue = missingSelected.value;
			selectedLabel = missingSelected.label;
			selectedSummaryLabel = missingSelected.label;
			selectData.splice(1, 0, {
				value: missingSelected.value,
				label: missingSelected.label,
			});
		}
	}

	return {
		defaultDeviceName,
		defaultOptionLabel,
		deviceOptions,
		selectData,
		selectedValue,
		selectedLabel,
		selectedSummaryLabel,
		selectedDevice,
		missingSelected,
		hasDevices: deviceOptions.length > 0,
		hasAnyDetectedInput: deviceOptions.length > 0 || defaultDeviceName !== null,
		legacySelectionTargetId,
	};
}

export function describeMicSelection(model: MicSelectorModel): string {
	if (!model.hasAnyDetectedInput) {
		return "Kolboo can’t currently see any input microphones.";
	}

	if (model.missingSelected) {
		return `Your saved microphone (${model.missingSelected.name}) isn’t available right now.`;
	}

	if (model.selectedValue === "default") {
		return model.defaultDeviceName
			? `Kolboo will use the current system default microphone: ${model.defaultDeviceName}.`
			: "Kolboo will use Windows’ current default microphone.";
	}

	return `Kolboo will record from ${model.selectedSummaryLabel}.`;
}

export function canRunMicTest(model: MicSelectorModel): boolean {
	return model.hasAnyDetectedInput && !model.missingSelected;
}

export function micPeakToDbfs(peak: number): number {
	const clamped = Math.max(0, Math.min(1, peak));
	return 20 * Math.log10(Math.max(clamped, 1e-4));
}

export function micPeakToMeterLevel(peak: number): number {
	// Voice tends to live well below 0 dBFS, so the UI intentionally stretches
	// the -55..-5 dBFS band to make normal speech visible instead of flat.
	const dbfs = micPeakToDbfs(peak);
	const minDb = -55;
	const maxDb = -5;
	const t = (dbfs - minDb) / (maxDb - minDb);
	return Math.max(0, Math.min(1, t)) ** 0.75;
}

export function micPeakToMeterColor(peak: number): string {
	const dbfs = micPeakToDbfs(peak);
	if (dbfs >= -3) return "#ef4444";
	if (dbfs >= -12) return "#f59e0b";
	return "#22c55e";
}

export function toMicTestErrorMessage(error: unknown): string {
	const message = formatErrorMessage(error).trim();
	const lowered = message.toLowerCase();

	if (lowered.includes("cannot test microphone level while recording")) {
		return "Stop the current recording before testing your microphone.";
	}

	if (lowered.includes("no input device")) {
		return "Kolboo couldn’t find a microphone to test. Plug one in, check Windows sound settings, then refresh the list.";
	}

	if (lowered.includes("failed to start mic test")) {
		return "Kolboo couldn’t start the microphone test. Try refreshing the device list or choosing System Default.";
	}

	return message.length > 0 && message !== "Unknown error"
		? message
		: "Kolboo couldn’t start the microphone test right now.";
}

export function toMicListErrorMessage(error: unknown): string {
	const message = formatErrorMessage(error).trim();

	if (message.length > 0 && message !== "Unknown error") {
		return `Kolboo couldn’t list microphones right now. ${message}`;
	}

	return "Kolboo couldn’t list microphones right now. Try refreshing, or reopen the app if Windows just changed audio devices.";
}
