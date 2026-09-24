import { describe, expect, it } from "vitest";
import {
	buildMicSelectorModel,
	decodeMicDeviceIdName,
	describeMicSelection,
	toMicListErrorMessage,
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

		expect(model.defaultOptionLabel).toBe("System default · USB Mic");
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
		expect(model.defaultOptionLabel).toBe("System default — unavailable");
		expect(describeMicSelection(model)).toBe(
			"Kolboo can’t currently see any input microphones.",
		);
	});

	it("avoids repeating generic OS names in the default microphone label", () => {
		for (const name of ["Default Audio Device", "default", "System Default"]) {
			const model = buildMicSelectorModel({
				devices: [{ id: "mic-1", name }],
				defaultDeviceName: name,
				storedMicId: null,
			});
			expect(model.defaultOptionLabel).toBe("System default");
			expect(model.selectedSummaryLabel).toBe("System default");
		}
	});

	it("does not claim a system default exists when only explicit inputs are known", () => {
		const model = buildMicSelectorModel({
			devices: [{ id: "mic-1", name: "USB Mic" }],
			defaultDeviceName: null,
			storedMicId: null,
		});
		expect(describeMicSelection(model)).toBe(
			"No system default microphone was detected. Choose an input from the list.",
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
			"Kolboo couldn’t find a microphone to test. Check your system’s input settings, then refresh the list.",
		);
	});

	it("gives platform-neutral guidance when device enumeration fails without detail", () => {
		expect(toMicListErrorMessage(undefined)).toBe(
			"Kolboo couldn’t list microphones right now. Try refreshing, or reopen the app after an audio-device change.",
		);
	});
});
