import { describe, expect, it } from "vitest";
import {
	buildMicSelectorModel,
	decodeMicDeviceIdName,
	describeMicSelection,
	toMicTestErrorMessage,
} from "./audioDevices";

describe("audioDevices read model", () => {
	it("disambiguates duplicate device names", () => {
		const model = buildMicSelectorModel({
			devices: [
				{ id: "mic-1", name: "USB Mic" },
				{ id: "mic-2", name: "USB Mic" },
			],
			defaultDeviceName: "USB Mic",
			storedMicId: "mic-2",
		});

		expect(model.defaultOptionLabel).toBe("System Default: USB Mic");
		expect(model.selectData.slice(1)).toEqual([
			{ value: "mic-1", label: "USB Mic · Device 1 of 2" },
			{ value: "mic-2", label: "USB Mic · Device 2 of 2" },
		]);
		expect(model.selectedLabel).toBe("USB Mic · Device 2 of 2");
	});

	it("keeps a helpful no-default label when no devices exist", () => {
		const model = buildMicSelectorModel({
			devices: [],
			defaultDeviceName: null,
			storedMicId: null,
		});

		expect(model.hasAnyDetectedInput).toBe(false);
		expect(model.defaultOptionLabel).toBe(
			"System Default — no default detected",
		);
		expect(describeMicSelection(model)).toBe(
			"Kolboo can’t currently see any input microphones.",
		);
	});

	it("decodes missing encoded microphone selections", () => {
		const encoded = `mic:v1:${Buffer.from("Studio Mic").toString("base64url")}:0`;
		const model = buildMicSelectorModel({
			devices: [{ id: "mic-1", name: "Desk Mic" }],
			defaultDeviceName: "Desk Mic",
			storedMicId: encoded,
		});

		expect(decodeMicDeviceIdName(encoded)).toBe("Studio Mic");
		expect(model.missingSelected).toEqual({
			value: encoded,
			label: "Missing microphone: Studio Mic",
			name: "Studio Mic",
		});
		expect(model.selectData[1]).toEqual({
			value: encoded,
			label: "Missing microphone: Studio Mic",
		});
	});

	it("maps legacy stored names to the first matching encoded device id", () => {
		const model = buildMicSelectorModel({
			devices: [
				{ id: "mic-1", name: "Boom Mic" },
				{ id: "mic-2", name: "Boom Mic" },
			],
			defaultDeviceName: "Boom Mic",
			storedMicId: "Boom Mic",
		});

		expect(model.legacySelectionTargetId).toBe("mic-1");
		expect(model.selectedValue).toBe("mic-1");
		expect(model.selectedSummaryLabel).toBe("Boom Mic · Device 1 of 2");
	});

	it("maps mic test errors into troubleshooting copy", () => {
		expect(
			toMicTestErrorMessage("Cannot test microphone level while recording."),
		).toBe("Stop the current recording before testing your microphone.");

		expect(
			toMicTestErrorMessage({ message: "No input device available" }),
		).toBe(
			"Kolboo couldn’t find a microphone to test. Plug one in, check Windows sound settings, then refresh the list.",
		);
	});
});
