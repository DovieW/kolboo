import { Loader, ScrollArea, Text } from "@mantine/core";
import { invoke } from "@tauri-apps/api/core";
import {
	getCurrentWindow,
	LogicalSize,
	PhysicalPosition,
} from "@tauri-apps/api/window";
import { openUrl } from "@tauri-apps/plugin-opener";
import hljs from "highlight.js/lib/core";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { useModifierKeyForwarder } from "./hooks/useModifierKeyForwarder";
import { applyAccentColor } from "./lib/accentColor";
import { readBootAccentColor } from "./lib/bootStorage";
import type {
	AppSettings,
	QuickAskAnswerPayload,
	QuickAskDismissMode,
	QuickAskStartedPayload,
} from "./lib/tauri";
import { tauriAPI } from "./lib/tauri";
import { useBackendEvent } from "./lib/tauri/useBackendEvent";
import { QuickAskStream } from "./lib/quickAskStream";
import "./app.css";

function sanitizeExternalHref(href: string | undefined | null): string | null {
	if (typeof href !== "string") return null;
	const trimmed = href.trim();
	if (!trimmed) return null;

	// Disallow obviously dangerous schemes.
	const lower = trimmed.toLowerCase();
	if (
		lower.startsWith("javascript:") ||
		lower.startsWith("data:") ||
		lower.startsWith("vbscript:")
	) {
		return null;
	}

	// Allow mailto: directly.
	if (lower.startsWith("mailto:")) return trimmed;

	// Only allow absolute http(s) URLs.
	try {
		const u = new URL(trimmed);
		if (u.protocol === "http:" || u.protocol === "https:") return u.toString();
		return null;
	} catch {
		return null;
	}
}

function highlightCodeHtml(code: string, language: string): string {
	try {
		if (language !== "plaintext" && hljs.getLanguage(language)) {
			return hljs.highlight(code, { language, ignoreIllegals: true }).value;
		}
		return hljs.highlightAuto(code).value;
	} catch {
		try {
			return hljs.highlightAuto(code).value;
		} catch {
			// Last resort: escape by treating it as plaintext.
			return hljs.highlight(code, {
				language: "plaintext",
				ignoreIllegals: true,
			}).value;
		}
	}
}

function copyToClipboard(text: string): Promise<void> {
	// Best effort: use async clipboard API, fallback to execCommand.
	if (navigator.clipboard?.writeText) {
		return navigator.clipboard.writeText(text);
	}

	return new Promise((resolve, reject) => {
		try {
			const ta = document.createElement("textarea");
			ta.value = text;
			ta.setAttribute("readonly", "");
			ta.style.position = "fixed";
			ta.style.opacity = "0";
			ta.style.left = "-9999px";
			document.body.appendChild(ta);
			ta.select();
			const ok = document.execCommand("copy");
			document.body.removeChild(ta);
			if (!ok) {
				reject(new Error("Copy failed"));
				return;
			}
			resolve();
		} catch (err) {
			reject(err);
		}
	});
}

function QuickAskCodeBlock({
	code,
	language,
}: {
	code: string;
	language: string;
}) {
	const [copied, setCopied] = useState(false);
	const highlighted = useMemo(
		() => highlightCodeHtml(code, language),
		[code, language],
	);

	return (
		<div className="quick-ask-codeblock-wrap">
			<div className="quick-ask-codeblock-toolbar">
				<span className="quick-ask-codeblock-lang">{language}</span>
				<button
					type="button"
					className="quick-ask-codeblock-copy"
					onClick={() => {
						copyToClipboard(code)
							.then(() => {
								setCopied(true);
								window.setTimeout(() => setCopied(false), 900);
							})
							.catch(() => {
								// ignore
							});
					}}
					aria-label="Copy code"
				>
					{copied ? "Copied" : "Copy"}
				</button>
			</div>
			<pre className="quick-ask-codeblock">
				<code
					className={`hljs language-${language}`}
					// highlight.js returns escaped HTML with span wrappers.
					// We do NOT allow arbitrary raw HTML from markdown.
					// biome-ignore lint/security/noDangerouslySetInnerHtml: highlight.js output is escaped; we only add span wrappers.
					dangerouslySetInnerHTML={{ __html: highlighted }}
				/>
			</pre>
		</div>
	);
}

type ActiveProfileInfo = {
	profile_id: string | null;
	profile_name: string | null;
};

export default function QuickAskApp() {
	// Forward modifier-only key events (like AltRight) to the backend.
	// WebView2 intercepts these before our keyboard hook sees them.
	useModifierKeyForwarder();

	const win = useMemo(() => getCurrentWindow(), []);
	const [phase, setPhase] = useState<"idle" | "loading" | "ready" | "error">(
		"idle",
	);
	const stream = useRef(new QuickAskStream());
	const [closing, setClosing] = useState(false);
	const [question, setQuestion] = useState<string>("");
	const [answer, setAnswer] = useState<string>("");
	const [error, setError] = useState<string>("");
	const [panelKey, setPanelKey] = useState(0);
	const dismissTimerRef = useRef<number | null>(null);
	const panelRef = useRef<HTMLDivElement | null>(null);
	const lastWindowSizeRef = useRef<{ width: number; height: number } | null>(
		null,
	);
	const [answerScrollable, setAnswerScrollable] = useState(false);
	const [settings, setSettings] = useState<AppSettings | null>(null);
	const [activeProfileId, setActiveProfileId] = useState<string | null>(null);
	const setClickThrough = useCallback(
		(enabled: boolean) => {
			win.setIgnoreCursorEvents(enabled).catch(() => {
				// ignore
			});
		},
		[win],
	);
	const resizeWindowToPanel = useCallback(() => {
		const panel = panelRef.current;
		if (!panel) return;
		const backdrop = panel.parentElement;
		if (!backdrop) return;

		const rect = panel.getBoundingClientRect();
		const styles = window.getComputedStyle(backdrop);
		const padTop = Number.parseFloat(styles.paddingTop || "0") || 0;
		const padBottom = Number.parseFloat(styles.paddingBottom || "0") || 0;
		const targetWidth = Math.min(520, Math.max(320, rect.width));
		const targetHeight = Math.min(
			440 + padTop + padBottom,
			rect.height + padTop + padBottom,
		);

		const last = lastWindowSizeRef.current;
		if (
			last &&
			Math.abs(last.width - targetWidth) < 1 &&
			Math.abs(last.height - targetHeight) < 1
		) {
			return;
		}
		lastWindowSizeRef.current = { width: targetWidth, height: targetHeight };
		void (async () => {
			const [prevPos, prevSize, scale] = await Promise.all([
				win.outerPosition().catch(() => null),
				win.outerSize().catch(() => null),
				win.scaleFactor().catch(() => 1),
			]);

			await win
				.setSize(new LogicalSize(targetWidth, targetHeight))
				.catch(() => {
					// ignore
				});

			if (!prevPos || !prevSize) return;
			const nextHeight = Math.round(targetHeight * scale);
			const nextY = prevPos.y + (prevSize.height - nextHeight);
			await win
				.setPosition(new PhysicalPosition(prevPos.x, nextY))
				.catch(() => {
					// ignore
				});
		})();
	}, [win]);

	useEffect(() => {
		applyAccentColor(readBootAccentColor());
		setClickThrough(true);
	}, [setClickThrough]);

	useEffect(() => {
		setClickThrough(phase === "idle");
	}, [phase, setClickThrough]);

	// The answer key replaces this node. Own observation through the DOM ref,
	// rather than an effect that can remain attached to a detached old panel.
	const observePanel = useCallback(
		(panel: HTMLDivElement | null) => {
			panelRef.current = panel;
			if (!panel || typeof ResizeObserver === "undefined") return;
			let raf = 0;
			const observer = new ResizeObserver(() => {
				if (raf) cancelAnimationFrame(raf);
				raf = requestAnimationFrame(() => {
					resizeWindowToPanel();
				});
			});
			observer.observe(panel);
			return () => {
				if (raf) cancelAnimationFrame(raf);
				observer.disconnect();
				panelRef.current = null;
			};
		},
		[resizeWindowToPanel],
	);

	const clearState = useCallback(() => {
		setPhase("idle");
		setQuestion("");
		setAnswer("");
		setError("");
		setAnswerScrollable(false);
		setActiveProfileId(null);
	}, []);

	const cancelAnswer = useCallback(() => {
		const requestId = stream.current.cancel();
		if (requestId) void tauriAPI.cancelQuickAsk(requestId).catch(() => {});
	}, []);
	useEffect(() => () => cancelAnswer(), [cancelAnswer]);

	useBackendEvent("quick-ask-partial", (payload) => {
		const text = stream.current.partial(payload);
		if (text !== null) setAnswer(text);
	});

	const hideNow = useCallback(() => {
		if (dismissTimerRef.current) {
			window.clearTimeout(dismissTimerRef.current);
			dismissTimerRef.current = null;
		}
		cancelAnswer();
		const version = stream.current.version;
		win
			.hide()
			.catch(() => {
				// ignore
			})
			.finally(() => {
				if (stream.current.version !== version) return;
				setClickThrough(true);
				tauriAPI.setQuickAskEscapeEnabled(false).catch(() => {
					// ignore
				});
				setClosing(false);
				clearState();
			});
	}, [cancelAnswer, clearState, setClickThrough, win]);

	useEffect(() => {
		return () => {
			if (dismissTimerRef.current) {
				window.clearTimeout(dismissTimerRef.current);
				dismissTimerRef.current = null;
			}
		};
	}, []);

	useEffect(() => {
		let cancelled = false;

		const loadSettings = async () => {
			try {
				await tauriAPI.reloadSettingsFromDisk();
			} catch {
				// ignore
			}

			try {
				const next = await tauriAPI.getSettings();
				if (!cancelled) setSettings(next);
			} catch {
				// ignore
			}
		};

		void loadSettings();

		let unlisten: (() => void) | undefined;
		const setup = async () => {
			unlisten = await tauriAPI.onSettingsChanged(() => {
				void loadSettings();
			});
		};

		void setup();

		return () => {
			cancelled = true;
			unlisten?.();
		};
	}, []);

	const resolveActiveProfileId = useCallback(async () => {
		const version = stream.current.version;
		try {
			const result = await invoke<ActiveProfileInfo>(
				"pipeline_get_active_profile_for_foreground_app",
			);
			if (stream.current.version === version)
				setActiveProfileId(result?.profile_id ?? null);
		} catch {
			console.warn("Quick Ask: failed to resolve active profile");
			if (stream.current.version === version) setActiveProfileId(null);
		}
	}, []);

	const dismiss = useCallback(() => {
		// Let CSS animate before hiding the window.
		if (dismissTimerRef.current) return;
		cancelAnswer();
		const version = stream.current.version;
		setClosing(true);
		window.requestAnimationFrame(() => {
			if (stream.current.version !== version) return;
			dismissTimerRef.current = window.setTimeout(() => {
				dismissTimerRef.current = null;
				win
					.hide()
					.catch(() => {
						// ignore
					})
					.finally(() => {
						if (stream.current.version !== version) return;
						setClickThrough(true);
						tauriAPI.setQuickAskEscapeEnabled(false).catch(() => {
							// ignore
						});
						setClosing(false);
						clearState();
					});
			}, 210);
		});
	}, [cancelAnswer, clearState, setClickThrough, win]);

	useBackendEvent("quick-ask-started", (payload) => {
		if (dismissTimerRef.current) {
			window.clearTimeout(dismissTimerRef.current);
			dismissTimerRef.current = null;
		}
		const safePayload: QuickAskStartedPayload =
			payload && typeof payload === "object" ? payload : {};
		stream.current.begin(safePayload.request_id ?? null);
		setPanelKey((k) => k + 1);
		setClosing(false);
		setPhase("loading");
		setQuestion(
			typeof safePayload.question === "string" ? safePayload.question : "",
		);
		setAnswer("");
		setError("");
		setAnswerScrollable(false);
		tauriAPI.setQuickAskEscapeEnabled(true).catch(() => {
			// ignore
		});
		setClickThrough(false);
		win.setAlwaysOnTop(true).catch(() => {
			// ignore
		});
		win.show().catch(() => {
			// ignore
		});
		win.setFocus().catch(() => {
			// ignore
		});
		resizeWindowToPanel();
		void resolveActiveProfileId();
	});

	useBackendEvent("quick-ask-answer", (payload) => {
		// If we aren't actively waiting for an answer, ignore late/stale events.
		const safePayload = payload as QuickAskAnswerPayload;
		if (!stream.current.finish(safePayload)) return;
		if (safePayload.ok) {
			setClosing(false);
			setPhase("ready");
			setAnswerScrollable(false);
			setAnswer(safePayload.answer);
			setError("");
			setClickThrough(false);
			win.setAlwaysOnTop(true).catch(() => {
				// ignore
			});
			resizeWindowToPanel();
			return;
		}

		setClosing(false);
		setPhase("error");
		setError(safePayload.error);
		setAnswerScrollable(false);
	});

	useBackendEvent("quick-ask-dismiss-requested", () => {
		dismiss();
	});

	useBackendEvent("recording-start", () => {
		hideNow();
	});

	useBackendEvent("pipeline-transcription-started", () => {
		hideNow();
	});
	const effectiveDismissMode: QuickAskDismissMode = useMemo(() => {
		if (!settings) return "manual";
		const profiles = settings.rewrite_program_prompt_profiles ?? [];
		const defaultProfile = profiles.find((p) => p.id === "default") ?? null;
		const base: QuickAskDismissMode =
			defaultProfile?.quick_ask_dismiss_mode ??
			settings.quick_ask_dismiss_mode ??
			"manual";

		if (!activeProfileId || activeProfileId === "default") return base;
		const activeProfile =
			profiles.find((p) => p.id === activeProfileId) ?? null;
		return activeProfile?.quick_ask_dismiss_mode ?? base;
	}, [activeProfileId, settings]);

	const allowClickAwayDismiss = effectiveDismissMode === "auto";

	useEffect(() => {
		const onKeyDown = (e: KeyboardEvent) => {
			if (e.key === "Escape") {
				dismiss();
			}
		};
		window.addEventListener("keydown", onKeyDown);
		return () => window.removeEventListener("keydown", onKeyDown);
	}, [dismiss]);

	useEffect(() => {
		if (!allowClickAwayDismiss) return;
		// Use window blur as a fallback "click-away" signal. In Tauri, clicks on
		// other windows or native chrome won't hit our backdrop mousedown handler,
		// but they do blur this window, so we auto-dismiss here when allowed.
		const onBlur = () => {
			dismiss();
		};
		window.addEventListener("blur", onBlur);
		return () => window.removeEventListener("blur", onBlur);
	}, [allowClickAwayDismiss, dismiss]);

	return (
		<div
			className={`quick-ask-backdrop${closing ? " closing" : ""}`}
			role="dialog"
			aria-label="Quick Ask answer"
			onMouseDown={(e) => {
				// Click outside the panel dismisses.
				if (e.target === e.currentTarget && allowClickAwayDismiss) {
					dismiss();
				}
			}}
		>
			<div
				className={`quick-ask-panel${closing ? " closing" : ""}`}
				key={panelKey}
				ref={observePanel}
			>
				{phase !== "idle" ? (
					<div className="quick-ask-question-row">
						<Text
							size="xs"
							c="dimmed"
							className="quick-ask-question"
							title={question}
						>
							{question || " "}
						</Text>
						<button
							type="button"
							className="quick-ask-close"
							aria-label="Close Quick Ask"
							title="Close"
							onClick={() => dismiss()}
							disabled={closing}
						>
							×
						</button>
					</div>
				) : null}

				{phase === "loading" ? (
					<div className="quick-ask-loading">
						<Loader size="sm" color="orange" />
						<Text size="sm" c="dimmed">
							{answer ? "Answering…" : "Thinking…"}
						</Text>
					</div>
				) : null}

				{phase === "error" ? (
					<Text size="sm" c="red" className="quick-ask-error">
						{answer ? "Answer incomplete: " : ""}
						{error}
					</Text>
				) : null}

				{answer ? (
					<ScrollArea.Autosize
						className={`quick-ask-answer${answerScrollable ? " quick-ask-answer--scrollable" : ""}`}
						type="auto"
						scrollbars="y"
						mah={320}
						onOverflowChange={setAnswerScrollable}
					>
						<div
							className="quick-ask-answer-md"
							dir="auto"
							aria-busy={phase === "loading"}
						>
							<ReactMarkdown
								remarkPlugins={[remarkGfm]}
								components={{
									a: ({ href, children, ...props }) => {
										const safeHref = sanitizeExternalHref(href);

										return (
											<a
												{...props}
												href={safeHref ?? undefined}
												target="_blank"
												rel="noreferrer noopener"
												onClick={(e) => {
													// Never navigate inside the webview.
													e.preventDefault();
													e.stopPropagation();
													if (!safeHref) return;
													openUrl(safeHref).catch(() => {
														// ignore
													});
												}}
											>
												{children}
											</a>
										);
									},
									code: ({ className, children, ...props }) => {
										const raw = String(children ?? "");
										const code = raw.endsWith("\n") ? raw.slice(0, -1) : raw;
										const m = /language-([a-zA-Z0-9_-]+)/.exec(className ?? "");
										const language = m?.[1] ?? "plaintext";

										// Inline code: leave it to CSS (we style it in app.css).
										// Block code: use CodeHighlight for syntax highlighting + copy.
										const isBlock = (className ?? "").includes("language-");
										if (!isBlock) {
											return (
												<code className={className} {...props}>
													{children}
												</code>
											);
										}

										return (
											<QuickAskCodeBlock code={code} language={language} />
										);
									},
								}}
							>
								{answer}
							</ReactMarkdown>
						</div>
					</ScrollArea.Autosize>
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
