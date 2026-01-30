import { describe, expect, it } from "vitest";
import {
	computeOcrPhaseText,
	computeOcrPillState,
	type OcrOverlayInputs,
} from "./ocrOverlayBehavior";

describe("computeOcrPillState", () => {
	const baseInputs: OcrOverlayInputs = {
		pipelineState: "recording",
		isError: false,
		ocrManualAvailable: true,
		ocrStatus: "not_started",
		ocrProviderAvailable: true,
		ocrProviderReason: null,
		sttComplete: false,
	};

	describe("visibility", () => {
		it("is visible when manual OCR is available and recording", () => {
			const result = computeOcrPillState(baseInputs);
			expect(result.visible).toBe(true);
		});

		it("is hidden when pipeline is idle", () => {
			const result = computeOcrPillState({
				...baseInputs,
				pipelineState: "idle",
			});
			expect(result.visible).toBe(false);
		});

		it("is hidden when there is an error", () => {
			const result = computeOcrPillState({
				...baseInputs,
				isError: true,
			});
			expect(result.visible).toBe(false);
		});

		it("is hidden when manual OCR is not available", () => {
			const result = computeOcrPillState({
				...baseInputs,
				ocrManualAvailable: false,
			});
			expect(result.visible).toBe(false);
		});

		it("is visible during transcribing phase", () => {
			const result = computeOcrPillState({
				...baseInputs,
				pipelineState: "transcribing",
			});
			expect(result.visible).toBe(true);
		});

		it("is visible during rewriting phase", () => {
			const result = computeOcrPillState({
				...baseInputs,
				pipelineState: "rewriting",
			});
			expect(result.visible).toBe(true);
		});
	});

	describe("clickability", () => {
		it("is clickable when visible, provider available, and not_started", () => {
			const result = computeOcrPillState(baseInputs);
			expect(result.canClick).toBe(true);
		});

		it("is not clickable when provider is unavailable", () => {
			const result = computeOcrPillState({
				...baseInputs,
				ocrProviderAvailable: false,
				ocrProviderReason: "OCR API key not set",
			});
			expect(result.canClick).toBe(false);
		});

		it("is not clickable when OCR is already running", () => {
			const result = computeOcrPillState({
				...baseInputs,
				ocrStatus: "running",
			});
			expect(result.canClick).toBe(false);
		});

		it("is not clickable when OCR is done", () => {
			const result = computeOcrPillState({
				...baseInputs,
				ocrStatus: "done",
			});
			expect(result.canClick).toBe(false);
		});

		it("is clickable when OCR previously failed", () => {
			const result = computeOcrPillState({
				...baseInputs,
				ocrStatus: "failed",
			});
			expect(result.canClick).toBe(true);
		});

		it("is clickable when OCR was cancelled", () => {
			const result = computeOcrPillState({
				...baseInputs,
				ocrStatus: "cancelled",
			});
			expect(result.canClick).toBe(true);
		});

		it("is not clickable when not visible (idle state)", () => {
			const result = computeOcrPillState({
				...baseInputs,
				pipelineState: "idle",
			});
			expect(result.canClick).toBe(false);
		});
	});

	describe("variant", () => {
		it('is "arming" (pulsing) when OCR is running', () => {
			const result = computeOcrPillState({
				...baseInputs,
				ocrStatus: "running",
			});
			expect(result.variant).toBe("arming");
		});

		it('is "dim" when OCR is not running', () => {
			const result = computeOcrPillState({
				...baseInputs,
				ocrStatus: "not_started",
			});
			expect(result.variant).toBe("dim");
		});

		it('is "dim" when OCR is done', () => {
			const result = computeOcrPillState({
				...baseInputs,
				ocrStatus: "done",
			});
			expect(result.variant).toBe("dim");
		});
	});

	describe("title (tooltip)", () => {
		it("shows unavailable reason when provider is unavailable", () => {
			const result = computeOcrPillState({
				...baseInputs,
				ocrProviderAvailable: false,
				ocrProviderReason: "OCR base URL not set",
			});
			expect(result.title).toBe("OCR base URL not set");
		});

		it('shows "Running OCR" when OCR is in progress', () => {
			const result = computeOcrPillState({
				...baseInputs,
				ocrStatus: "running",
			});
			expect(result.title).toBe("Running OCR");
		});

		it('shows "OCR ready" when OCR is done', () => {
			const result = computeOcrPillState({
				...baseInputs,
				ocrStatus: "done",
			});
			expect(result.title).toBe("OCR ready");
		});

		it('shows "Run OCR for the active window" when OCR can be started', () => {
			const result = computeOcrPillState(baseInputs);
			expect(result.title).toBe("Run OCR for the active window");
		});

		it("falls back to generic unavailable message when no reason given", () => {
			const result = computeOcrPillState({
				...baseInputs,
				ocrProviderAvailable: false,
				ocrProviderReason: null,
			});
			expect(result.title).toBe("OCR provider unavailable");
		});
	});

	describe("status flags", () => {
		it("isRunning is true when OCR status is running", () => {
			const result = computeOcrPillState({
				...baseInputs,
				ocrStatus: "running",
			});
			expect(result.isRunning).toBe(true);
			expect(result.isDone).toBe(false);
		});

		it("isDone is true when OCR status is done", () => {
			const result = computeOcrPillState({
				...baseInputs,
				ocrStatus: "done",
			});
			expect(result.isRunning).toBe(false);
			expect(result.isDone).toBe(true);
		});

		it("isBlocking is true when STT is complete and OCR is running", () => {
			const result = computeOcrPillState({
				...baseInputs,
				ocrStatus: "running",
				sttComplete: true,
			});
			expect(result.isBlocking).toBe(true);
		});

		it("isBlocking is false when STT is not complete", () => {
			const result = computeOcrPillState({
				...baseInputs,
				ocrStatus: "running",
				sttComplete: false,
			});
			expect(result.isBlocking).toBe(false);
		});

		it("isBlocking is false when OCR is not running", () => {
			const result = computeOcrPillState({
				...baseInputs,
				ocrStatus: "done",
				sttComplete: true,
			});
			expect(result.isBlocking).toBe(false);
		});
	});

	describe("null/undefined OCR status handling", () => {
		it("treats null status as not_started", () => {
			const result = computeOcrPillState({
				...baseInputs,
				ocrStatus: null,
			});
			expect(result.isRunning).toBe(false);
			expect(result.isDone).toBe(false);
			expect(result.canClick).toBe(true);
		});
	});
});

describe("computeOcrPhaseText", () => {
	const baseInputs = {
		pipelineState: "idle" as const,
		ocrStatus: null as string | null,
		sttComplete: false,
		overlayMode: "always" as string | null,
		holdPhaseText: null as "rewriting" | "routing" | "transcribing" | null,
		animState: "visible",
		lastBusyPhase: null as "rewriting" | "routing" | "transcribing" | null,
	};

	describe("OCR blocking indicator", () => {
		it('shows "OCR..." when STT is complete but OCR is still running', () => {
			const result = computeOcrPhaseText({
				...baseInputs,
				ocrStatus: "running",
				sttComplete: true,
			});
			expect(result).toBe("OCR...");
		});

		it("does not show OCR text when STT is not complete", () => {
			const result = computeOcrPhaseText({
				...baseInputs,
				ocrStatus: "running",
				sttComplete: false,
			});
			expect(result).toBeNull();
		});

		it("does not show OCR text when OCR is done", () => {
			const result = computeOcrPhaseText({
				...baseInputs,
				ocrStatus: "done",
				sttComplete: true,
			});
			expect(result).toBeNull();
		});
	});

	describe("pipeline phase text", () => {
		it('shows "rewriting..." during rewriting phase', () => {
			const result = computeOcrPhaseText({
				...baseInputs,
				pipelineState: "rewriting",
			});
			expect(result).toBe("rewriting...");
		});

		it('shows "routing..." during routing phase', () => {
			const result = computeOcrPhaseText({
				...baseInputs,
				pipelineState: "routing",
			});
			expect(result).toBe("routing...");
		});

		it('shows "transcribing..." during transcribing phase', () => {
			const result = computeOcrPhaseText({
				...baseInputs,
				pipelineState: "transcribing",
			});
			expect(result).toBe("transcribing...");
		});

		it("shows null during idle phase", () => {
			const result = computeOcrPhaseText({
				...baseInputs,
				pipelineState: "idle",
			});
			expect(result).toBeNull();
		});

		it("shows null during recording phase", () => {
			const result = computeOcrPhaseText({
				...baseInputs,
				pipelineState: "recording",
			});
			expect(result).toBeNull();
		});
	});

	describe("OCR blocking takes priority over pipeline phase", () => {
		it("shows OCR... even during rewriting if STT is done and OCR is running", () => {
			const result = computeOcrPhaseText({
				...baseInputs,
				pipelineState: "rewriting",
				ocrStatus: "running",
				sttComplete: true,
			});
			expect(result).toBe("OCR...");
		});
	});

	describe("recording-only mode holdPhaseText", () => {
		it("shows held rewriting text in recording_only mode", () => {
			const result = computeOcrPhaseText({
				...baseInputs,
				pipelineState: "idle",
				overlayMode: "recording_only",
				holdPhaseText: "rewriting",
			});
			expect(result).toBe("rewriting...");
		});

		it("shows held transcribing text in recording_only mode", () => {
			const result = computeOcrPhaseText({
				...baseInputs,
				pipelineState: "idle",
				overlayMode: "recording_only",
				holdPhaseText: "transcribing",
			});
			expect(result).toBe("transcribing...");
		});

		it("does not show held text in always mode", () => {
			const result = computeOcrPhaseText({
				...baseInputs,
				pipelineState: "idle",
				overlayMode: "always",
				holdPhaseText: "rewriting",
			});
			expect(result).toBeNull();
		});
	});

	describe("exit animation lastBusyPhase", () => {
		it("shows last busy phase during exit animation", () => {
			const result = computeOcrPhaseText({
				...baseInputs,
				pipelineState: "idle",
				animState: "exit",
				lastBusyPhase: "routing",
			});
			expect(result).toBe("routing...");
		});

		it("does not show last busy phase when not exiting", () => {
			const result = computeOcrPhaseText({
				...baseInputs,
				pipelineState: "idle",
				animState: "visible",
				lastBusyPhase: "routing",
			});
			expect(result).toBeNull();
		});
	});
});
