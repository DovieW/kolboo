import { Loader, ScrollArea, Text } from "@mantine/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { openUrl } from "@tauri-apps/plugin-opener";
import hljs from "highlight.js/lib/core";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { applyAccentColor } from "./lib/accentColor";
import { readBootAccentColor } from "./lib/bootStorage";
import type {
	QuickAskAnswerPayload,
	QuickAskStartedPayload,
} from "./lib/tauri";
import { listenTyped } from "./lib/tauri/events";
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

export default function QuickAskApp() {
	const win = useMemo(() => getCurrentWindow(), []);
	const [phase, setPhase] = useState<"idle" | "loading" | "ready" | "error">(
		"idle",
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

		listenTyped("quick-ask-started", (payload) => {
			const safePayload: QuickAskStartedPayload =
				payload && typeof payload === "object" ? payload : {};
			awaitingAnswerRef.current = true;
			setPanelKey((k) => k + 1);
			setClosing(false);
			setPhase("loading");
			setQuestion(
				typeof safePayload.question === "string" ? safePayload.question : "",
			);
			setAnswer("");
			setError("");
		})
			.then((fn) => {
				unlistenStarted = fn;
			})
			.catch(() => {
				// ignore
			});

		listenTyped("quick-ask-answer", (payload) => {
			// If we aren't actively waiting for an answer, ignore late/stale events.
			if (!awaitingAnswerRef.current) return;
			awaitingAnswerRef.current = false;
			const safePayload = payload as QuickAskAnswerPayload;
			if (
				safePayload &&
				typeof safePayload === "object" &&
				"ok" in safePayload
			) {
				if (safePayload.ok) {
					setClosing(false);
					setPhase("ready");
					setAnswer(safePayload.answer);
					setError("");
					return;
				}
			}

			setClosing(false);
			setPhase("error");
			setAnswer("");
			setError(
				safePayload && typeof safePayload === "object" && "error" in safePayload
					? String((safePayload as { error?: string }).error ?? "Unknown error")
					: "Unknown error",
			);
		})
			.then((fn) => {
				unlistenAnswer = fn;
			})
			.catch(() => {
				// ignore
			});

		// Starting a new recording/transcription should dismiss Quick Ask.
		listenTyped("recording-start", () => {
			hideNow();
		})
			.then((fn) => {
				unlistenRecordingStart = fn;
			})
			.catch(() => {
				// ignore
			});

		listenTyped("pipeline-transcription-started", () => {
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
			>
				{question ? (
					<Text
						size="xs"
						c="dimmed"
						className="quick-ask-question"
						title={question}
					>
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
					<ScrollArea
						className="quick-ask-answer"
						type="auto"
						scrollbars="y"
						mah={320}
					>
						<div className="quick-ask-answer-md">
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
