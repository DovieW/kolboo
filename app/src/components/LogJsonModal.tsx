import { CodeHighlight } from "@mantine/code-highlight";
import { Box, Modal, Stack, Tabs, Text } from "@mantine/core";
import { useEffect, useMemo, useState } from "react";
import type { RequestLog } from "../lib/tauri";

type TabKey =
	| "full"
	| "stt-request"
	| "stt-response"
	| "llm-request"
	| "llm-response"
	| "ocr-request"
	| "ocr-response"
	| "quick-ask-request"
	| "quick-ask-response"
	| "quick-replace-request"
	| "quick-replace-response"
	| "router-request"
	| "router-response";

function stringifyJson(value: unknown): string {
	if (value === undefined) return "";
	if (value === null) return "null";

	if (typeof value === "string") {
		const trimmed = value.trim();
		// If it looks like JSON, pretty print it.
		if (
			(trimmed.startsWith("{") && trimmed.endsWith("}")) ||
			(trimmed.startsWith("[") && trimmed.endsWith("]"))
		) {
			try {
				return JSON.stringify(JSON.parse(trimmed), null, 2);
			} catch {
				return value;
			}
		}
		return value;
	}

	try {
		return JSON.stringify(value, null, 2);
	} catch {
		return String(value);
	}
}

function JsonPanel({ value }: { value: unknown }) {
	const code = useMemo(() => stringifyJson(value), [value]);

	if (!code || code.trim().length === 0) {
		return (
			<Text size="sm" c="dimmed">
				No payload captured for this request.
			</Text>
		);
	}

	return (
		<Box
			style={{
				height: "100%",
				minHeight: 0,
				overflow: "hidden",
			}}
		>
			<CodeHighlight
				code={code}
				language="json"
				copyLabel="Copy payload"
				copiedLabel="Copied"
				styles={{
					codeHighlight: {
						height: "100%",
						display: "flex",
						flexDirection: "column",
					},
					scrollarea: {
						flex: 1,
					},
				}}
			/>
		</Box>
	);
}

function redactPayloadValue(
	value: unknown,
	redactedKeys: Set<string>,
): unknown {
	if (value == null) return value;
	if (typeof value !== "object") return value;

	if (Array.isArray(value)) {
		return value.map((item) => redactPayloadValue(item, redactedKeys));
	}

	const record = value as Record<string, unknown>;
	const next: Record<string, unknown> = {};
	for (const [key, entry] of Object.entries(record)) {
		if (redactedKeys.has(key)) {
			next[key] = "<sensitive content omitted>";
			continue;
		}
		// Legacy/debug adapters may store a serialized body rather than an object.
		// Do not let formatting parse it after redaction and expose its content.
		if (key.endsWith("_json") && typeof entry === "string") {
			try {
				const parsed: unknown = JSON.parse(entry);
				next[key] =
					parsed !== null && typeof parsed === "object"
						? redactPayloadValue(parsed, redactedKeys)
						: "<unstructured payload omitted>";
			} catch {
				next[key] = "<unstructured payload omitted>";
			}
			continue;
		}
		next[key] = redactPayloadValue(entry, redactedKeys);
	}
	return next;
}

export default function LogJsonModal({
	opened,
	onClose,
	log,
}: {
	opened: boolean;
	onClose: () => void;
	log: RequestLog;
}) {
	const redactedKeys = useMemo(
		() =>
			new Set([
				"raw_transcript",
				"final_text",
				"formatted_transcript",
				"transcript",
				"text",
				"content",
				"output",
				"messages",
				"prompt",
				"input",
				"user_message",
				"system_prompt",
				"stt_transcription_prompt",
				"question",
				"answer",
				"context",
				"context_text",
				"clipboard_context",
				"ocr_context_text",
				"rewrite_clipboard_context",
				"quick_ask_question",
				"quick_ask_answer",
				"quick_ask_context_text",
				"quick_ask_clipboard_context",
				"quick_replace_instructions",
				"quick_replace_selected_text",
				"quick_replace_output_text",
				"quick_replace_clipboard_context",
				"selected_text",
				"instructions",
			]),
		[],
	);

	const logForModal = useMemo(
		() => redactPayloadValue(log, redactedKeys) as RequestLog,
		[log, redactedKeys],
	);
	const hasSttPayload =
		log.stt_request_json !== undefined || log.stt_response_json !== undefined;
	const hasLlmPayload =
		log.llm_request_json !== undefined || log.llm_response_json !== undefined;
	const hasQuickAskPayload =
		log.quick_ask_request_json !== undefined ||
		log.quick_ask_response_json !== undefined;
	const hasQuickReplacePayload =
		log.quick_replace_request_json !== undefined ||
		log.quick_replace_response_json !== undefined;
	const hasOcrPayload =
		log.ocr_request_json !== undefined || log.ocr_response_json !== undefined;
	const hasRouterPayload =
		log.router_request_json !== undefined ||
		log.router_response_json !== undefined;

	const [tab, setTab] = useState<TabKey>("full");

	// Reset selection when opening / switching log rows.
	useEffect(() => {
		if (!opened) return;
		void log.id;
		setTab("full");
	}, [opened, log.id]);

	return (
		<Modal
			opened={opened}
			onClose={onClose}
			title="Payloads"
			size="xl"
			centered
			overlayProps={{ opacity: 0.55, blur: 2 }}
			styles={{
				content: {
					height: "min(900px, 85vh)",
					display: "flex",
					flexDirection: "column",
					overflow: "hidden",
				},
				body: {
					flex: 1,
					display: "flex",
					flexDirection: "column",
					overflow: "hidden",
				},
			}}
		>
			<Stack gap="sm" style={{ flex: 1, minHeight: 0, overflow: "hidden" }}>
				<Text size="xs" c="dimmed">
					This debugging view omits sensitive content fields. STT requests use
					previews with binary audio omitted. Quick Ask/Quick Replace show
					logical metadata, not necessarily the provider’s wire format. This
					does not change what is sent to your selected provider or what you
					choose to save in History.
				</Text>

				<Tabs
					value={tab}
					onChange={(v) => setTab((v as TabKey) ?? "full")}
					style={{
						flex: 1,
						minHeight: 0,
						display: "flex",
						flexDirection: "column",
						overflow: "hidden",
					}}
				>
					<Tabs.List>
						<Tabs.Tab value="full">Log</Tabs.Tab>

						{hasSttPayload && (
							<>
								<Tabs.Tab value="stt-request">STT Request (preview)</Tabs.Tab>
								<Tabs.Tab value="stt-response">STT Response</Tabs.Tab>
							</>
						)}

						{hasLlmPayload && (
							<>
								<Tabs.Tab value="llm-request">LLM Request</Tabs.Tab>
								<Tabs.Tab value="llm-response">LLM Response</Tabs.Tab>
							</>
						)}

						{hasOcrPayload && (
							<>
								<Tabs.Tab value="ocr-request">OCR Request (preview)</Tabs.Tab>
								<Tabs.Tab value="ocr-response">OCR Response</Tabs.Tab>
							</>
						)}

						{hasQuickAskPayload && (
							<>
								<Tabs.Tab value="quick-ask-request">
									Quick Ask (logical)
								</Tabs.Tab>
								<Tabs.Tab value="quick-ask-response">Quick Ask Result</Tabs.Tab>
							</>
						)}

						{hasQuickReplacePayload && (
							<>
								<Tabs.Tab value="quick-replace-request">
									Quick Replace (logical)
								</Tabs.Tab>
								<Tabs.Tab value="quick-replace-response">
									Quick Replace Result
								</Tabs.Tab>
							</>
						)}

						{hasRouterPayload && (
							<>
								<Tabs.Tab value="router-request">Router (redacted)</Tabs.Tab>
								<Tabs.Tab value="router-response">Router Result</Tabs.Tab>
							</>
						)}
					</Tabs.List>

					<Tabs.Panel
						value="full"
						pt="sm"
						style={{ flex: 1, minHeight: 0, overflow: "hidden" }}
					>
						<JsonPanel value={logForModal} />
					</Tabs.Panel>

					{hasSttPayload && (
						<>
							<Tabs.Panel
								value="stt-request"
								pt="sm"
								style={{ flex: 1, minHeight: 0, overflow: "hidden" }}
							>
								<JsonPanel value={logForModal.stt_request_json} />
							</Tabs.Panel>
							<Tabs.Panel
								value="stt-response"
								pt="sm"
								style={{ flex: 1, minHeight: 0, overflow: "hidden" }}
							>
								<JsonPanel value={logForModal.stt_response_json} />
							</Tabs.Panel>
						</>
					)}

					{hasLlmPayload && (
						<>
							<Tabs.Panel
								value="llm-request"
								pt="sm"
								style={{ flex: 1, minHeight: 0, overflow: "hidden" }}
							>
								<JsonPanel value={logForModal.llm_request_json} />
							</Tabs.Panel>
							<Tabs.Panel
								value="llm-response"
								pt="sm"
								style={{ flex: 1, minHeight: 0, overflow: "hidden" }}
							>
								<JsonPanel value={logForModal.llm_response_json} />
							</Tabs.Panel>
						</>
					)}

					{hasOcrPayload && (
						<>
							<Tabs.Panel
								value="ocr-request"
								pt="sm"
								style={{ flex: 1, minHeight: 0, overflow: "hidden" }}
							>
								<JsonPanel value={logForModal.ocr_request_json} />
							</Tabs.Panel>
							<Tabs.Panel
								value="ocr-response"
								pt="sm"
								style={{ flex: 1, minHeight: 0, overflow: "hidden" }}
							>
								<JsonPanel value={logForModal.ocr_response_json} />
							</Tabs.Panel>
						</>
					)}

					{hasQuickAskPayload && (
						<>
							<Tabs.Panel
								value="quick-ask-request"
								pt="sm"
								style={{ flex: 1, minHeight: 0, overflow: "hidden" }}
							>
								<JsonPanel value={logForModal.quick_ask_request_json} />
							</Tabs.Panel>
							<Tabs.Panel
								value="quick-ask-response"
								pt="sm"
								style={{ flex: 1, minHeight: 0, overflow: "hidden" }}
							>
								<JsonPanel value={logForModal.quick_ask_response_json} />
							</Tabs.Panel>
						</>
					)}

					{hasQuickReplacePayload && (
						<>
							<Tabs.Panel
								value="quick-replace-request"
								pt="sm"
								style={{ flex: 1, minHeight: 0, overflow: "hidden" }}
							>
								<JsonPanel value={logForModal.quick_replace_request_json} />
							</Tabs.Panel>
							<Tabs.Panel
								value="quick-replace-response"
								pt="sm"
								style={{ flex: 1, minHeight: 0, overflow: "hidden" }}
							>
								<JsonPanel value={logForModal.quick_replace_response_json} />
							</Tabs.Panel>
						</>
					)}

					{hasRouterPayload && (
						<>
							<Tabs.Panel
								value="router-request"
								pt="sm"
								style={{ flex: 1, minHeight: 0, overflow: "hidden" }}
							>
								<JsonPanel value={logForModal.router_request_json} />
							</Tabs.Panel>
							<Tabs.Panel
								value="router-response"
								pt="sm"
								style={{ flex: 1, minHeight: 0, overflow: "hidden" }}
							>
								<JsonPanel value={logForModal.router_response_json} />
							</Tabs.Panel>
						</>
					)}
				</Tabs>

				{!hasSttPayload &&
					!hasLlmPayload &&
					!hasOcrPayload &&
					!hasQuickAskPayload &&
					!hasQuickReplacePayload &&
					!hasRouterPayload && (
						<Text size="xs" c="dimmed">
							No STT/LLM/OCR/Quick Ask/Quick Replace/Router payloads captured
							for this request.
						</Text>
					)}
			</Stack>
		</Modal>
	);
}
