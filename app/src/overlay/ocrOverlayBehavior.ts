import type { PipelineState } from "../lib/overlay/overlayUiReducer";

export type OcrStatus =
  | "not_started"
  | "running"
  | "done"
  | "failed"
  | "cancelled";

export interface OcrOverlayInputs {
  pipelineState: PipelineState;
  isError: boolean;
  ocrManualAvailable: boolean;
  ocrStatus: OcrStatus | string | null;
  ocrProviderAvailable: boolean;
  ocrProviderReason: string | null;
  sttComplete: boolean;
}

export interface OcrPillState {
  /** Whether the OCR pill should be visible in the overlay. */
  visible: boolean;
  /** Whether the OCR pill can be clicked to trigger OCR. */
  canClick: boolean;
  /** Visual variant for the pill: "arming" when running, "dim" otherwise. */
  variant: "arming" | "dim";
  /** Human-readable tooltip for the pill. */
  title: string;
  /** Whether OCR is currently running. */
  isRunning: boolean;
  /** Whether OCR is done (checkmark icon). */
  isDone: boolean;
  /** Whether OCR is blocking the pipeline (waiting for OCR to complete). */
  isBlocking: boolean;
}

/**
 * Computes the OCR pill UI state from overlay/pipeline state.
 * Extracted for testability.
 */
export function computeOcrPillState(inputs: OcrOverlayInputs): OcrPillState {
  const {
    pipelineState,
    isError,
    ocrManualAvailable,
    ocrStatus,
    ocrProviderAvailable,
    ocrProviderReason,
    sttComplete,
  } = inputs;

  const status = (ocrStatus ?? "not_started") as OcrStatus;
  const isRunning = status === "running";
  const isDone = status === "done";
  const isBlocking = isRunning && sttComplete;

  // Pill visibility: show when manual mode is available, not error, and not idle
  const visible = !isError && ocrManualAvailable && pipelineState !== "idle";

  // Pill is clickable when:
  // - visible
  // - provider is available
  // - OCR is in a state that allows starting (not_started, failed, cancelled)
  const canClick =
    visible &&
    ocrProviderAvailable &&
    (status === "not_started" || status === "failed" || status === "cancelled");

  // Variant: pulsing/arming when running, dim otherwise
  const variant: "arming" | "dim" = isRunning ? "arming" : "dim";

  // Title/tooltip
  let title: string;
  if (!ocrProviderAvailable) {
    title = ocrProviderReason || "OCR provider unavailable";
  } else if (isRunning) {
    title = "Running OCR";
  } else if (isDone) {
    title = "OCR ready";
  } else {
    title = "Run OCR for the active window";
  }

  return {
    visible,
    canClick,
    variant,
    title,
    isRunning,
    isDone,
    isBlocking,
  };
}

/**
 * Computes the phase text to show in the overlay center based on pipeline + OCR state.
 * Returns null if the normal waveform UI should be shown.
 */
export function computeOcrPhaseText(inputs: {
  pipelineState: PipelineState;
  ocrStatus: OcrStatus | string | null;
  sttComplete: boolean;
  overlayMode: string | null;
  holdPhaseText: PipelineState | null;
  animState: string;
  lastBusyPhase: PipelineState | null;
}): string | null {
  const {
    pipelineState,
    ocrStatus,
    sttComplete,
    overlayMode,
    holdPhaseText,
    animState,
    lastBusyPhase,
  } = inputs;

  const isOcrRunning = ocrStatus === "running";
  const ocrBlocking = isOcrRunning && sttComplete;

  // If STT is done but OCR is still running, show "OCR..." to indicate we're waiting.
  if (ocrBlocking) return "OCR...";

  if (pipelineState === "rewriting") return "rewriting...";
  if (pipelineState === "routing") return "routing...";
  if (pipelineState === "transcribing") return "transcribing...";

  // Recording-only: keep the last busy phase visible across the small idle gap
  if (overlayMode === "recording_only") {
    if (holdPhaseText === "rewriting") return "rewriting...";
    if (holdPhaseText === "routing") return "routing...";
    if (holdPhaseText === "transcribing") return "transcribing...";
  }

  // While fading out, keep showing the last busy phase
  if (animState === "exit") {
    if (lastBusyPhase === "rewriting") return "rewriting...";
    if (lastBusyPhase === "routing") return "routing...";
    if (lastBusyPhase === "transcribing") return "transcribing...";
  }

  return null;
}
