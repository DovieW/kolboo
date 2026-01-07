import { Loader, ScrollArea, Text } from "@mantine/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { applyAccentColor } from "./lib/accentColor";
import "./app.css";

function readBootAccentColor(): string | null {
	try {
		if (typeof window === "undefined" || !window.localStorage) return null;
		const raw = window.localStorage.getItem("tv_accent_color");
		if (typeof raw !== "string") return null;
		if (/^#([0-9a-fA-F]{6})$/.test(raw)) return raw;
		return null;
	} catch {
		return null;
	}
}

type QuickAskStartedPayload = {
	question?: string;
	provider?: string;
	model?: string | null;
};

type QuickAskAnswerPayload =
	| {
			ok: true;
			answer: string;
			provider_used?: string;
			model_used?: string;
			duration_ms?: number;
	  }
	| {
			ok: false;
			error: string;
	  };

export default function QuickAskApp() {
	const win = useMemo(() => getCurrentWindow(), []);
	const [phase, setPhase] = useState<"idle" | "loading" | "ready" | "error">(
		"idle"
	);
	const awaitingAnswerRef = useRef(false);
	const [closing, setClosing] = useState(false);
	const [question, setQuestion] = useState<string>("");
	const [answer, setAnswer] = useState<string>("");
	const [error, setError] = useState<string>("");
	const [panelKey, setPanelKey] = useState(0);
	const dismissTimerRef = useRef<number | null>(null);

	useEffect(() => {
		applyAccentColor(readBootAccentColor());
	}, []);

	const clearState = useCallback(() => {
		setPhase("idle");
		setQuestion("");
		setAnswer("");
		setError("");
	}, []);

	const hideNow = useCallback(() => {
		if (dismissTimerRef.current) {
			window.clearTimeout(dismissTimerRef.current);
			dismissTimerRef.current = null;
		}
		awaitingAnswerRef.current = false;
		win
			.hide()
			.catch(() => {
				// ignore
			})
			.finally(() => {
				setClosing(false);
				clearState();
			});
	}, [clearState, win]);

	useEffect(() => {
		return () => {
			if (dismissTimerRef.current) {
				window.clearTimeout(dismissTimerRef.current);
				dismissTimerRef.current = null;
			}
		};
	}, []);

	useEffect(() => {
		let unlistenStarted: (() => void) | null = null;
		let unlistenAnswer: (() => void) | null = null;
		let unlistenRecordingStart: (() => void) | null = null;
		let unlistenTranscriptionStart: (() => void) | null = null;

		listen<QuickAskStartedPayload>("quick-ask-started", (evt) => {
			const p = (evt.payload ?? {}) as QuickAskStartedPayload;
			awaitingAnswerRef.current = true;
			setPanelKey((k) => k + 1);
			setClosing(false);
			setPhase("loading");
			setQuestion(typeof p.question === "string" ? p.question : "");
			setAnswer("");
			setError("");
		})
			.then((fn) => {
				unlistenStarted = fn;
			})
			.catch(() => {
				// ignore
			});

		listen<QuickAskAnswerPayload>("quick-ask-answer", (evt) => {
			// If we aren't actively waiting for an answer, ignore late/stale events.
			if (!awaitingAnswerRef.current) return;
			awaitingAnswerRef.current = false;
			const p = (evt.payload ?? {}) as QuickAskAnswerPayload;
			if (p && typeof p === "object" && (p as any).ok === true) {
				setClosing(false);
				setPhase("ready");
				setAnswer(typeof (p as any).answer === "string" ? (p as any).answer : "");
				setError("");
				return;
			}

			setClosing(false);
			setPhase("error");
			setAnswer("");
			setError(typeof (p as any).error === "string" ? (p as any).error : "Unknown error");
		})
			.then((fn) => {
				unlistenAnswer = fn;
			})
			.catch(() => {
				// ignore
			});

		// Starting a new recording/transcription should dismiss Quick Ask.
		listen("recording-start", () => {
			hideNow();
		})
			.then((fn) => {
				unlistenRecordingStart = fn;
			})
			.catch(() => {
				// ignore
			});

		listen("pipeline-transcription-started", () => {
			hideNow();
		})
			.then((fn) => {
				unlistenTranscriptionStart = fn;
			})
			.catch(() => {
				// ignore
			});

		return () => {
			unlistenStarted?.();
			unlistenAnswer?.();
			unlistenRecordingStart?.();
			unlistenTranscriptionStart?.();
		};
	}, [hideNow]);

	const dismiss = useCallback(() => {
		// Let CSS animate before hiding the window.
		if (dismissTimerRef.current) return;
		awaitingAnswerRef.current = false;
		setClosing(true);
		window.requestAnimationFrame(() => {
			dismissTimerRef.current = window.setTimeout(() => {
				dismissTimerRef.current = null;
				win
					.hide()
					.catch(() => {
						// ignore
					})
					.finally(() => {
						setClosing(false);
						clearState();
					});
			}, 210);
		});
	}, [clearState, win]);

	useEffect(() => {
		const onKeyDown = (e: KeyboardEvent) => {
			if (e.key === "Escape") {
				dismiss();
			}
		};
		window.addEventListener("keydown", onKeyDown);
		return () => window.removeEventListener("keydown", onKeyDown);
	}, [dismiss]);

	return (
		<div
			className={`quick-ask-backdrop${closing ? " closing" : ""}`}
			role="dialog"
			aria-label="Quick Ask answer"
			onMouseDown={(e) => {
				// Click outside the panel dismisses.
				if (e.target === e.currentTarget) {
					dismiss();
				}
			}}
		>
			<div
				className={`quick-ask-panel${closing ? " closing" : ""}`}
				key={panelKey}
				onMouseDown={(e) => {
					// Prevent the backdrop click handler.
					e.stopPropagation();
				}}
			>
				{question ? (
					<Text size="xs" c="dimmed" className="quick-ask-question" title={question}>
						{question}
					</Text>
				) : null}

				{phase === "loading" ? (
					<div className="quick-ask-loading">
						<Loader size="sm" color="orange" />
						<Text size="sm" c="dimmed">
							Thinking…
						</Text>
					</div>
				) : null}

				{phase === "error" ? (
					<Text size="sm" c="red" className="quick-ask-error">
						{error}
					</Text>
				) : null}

				{phase === "ready" ? (
					<ScrollArea className="quick-ask-answer" type="auto" scrollbars="y">
						<Text size="sm" className="quick-ask-answer-text">
							{answer}
						</Text>
					</ScrollArea>
				) : null}

				{phase === "idle" ? (
					<Text size="sm" c="dimmed" className="quick-ask-hint">
						Waiting for Quick Ask…
					</Text>
				) : null}
			</div>
		</div>
	);
}
