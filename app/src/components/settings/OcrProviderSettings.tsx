import {
	Accordion,
	Button,
	Group,
	NumberInput,
	PasswordInput,
	Select,
	Switch,
	Textarea,
	TextInput,
	Tooltip,
} from "@mantine/core";
import { useQuery } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import {
	useClearOcrApiKey,
	useSetOcrApiKey,
	useSettings,
	useUpdateOcrAuthMode,
	useUpdateOcrAutoCaptureTiming,
	useUpdateOcrBaseUrl,
	useUpdateOcrContextMaxChars,
	useUpdateOcrHallucinationProtection,
	useUpdateOcrHallucinationThreshold,
	useUpdateOcrMaxTokens,
	useUpdateOcrModel,
	useUpdateOcrPrompt,
	useUpdateOcrRequestTimeoutMs,
	useUpdateOcrResizeFilter,
	useUpdateOcrResizeMaxDimension,
	useUpdateOcrTemperature,
	useUpdateOcrTopP,
} from "../../lib/queries";
import { tauriAPI } from "../../lib/tauri";
import { SettingsRow } from "./SettingsRow";

const GLOBAL_ONLY_TOOLTIP =
	"This setting can only be changed in the Default profile";

export function OcrProviderSettings({
	editingProfileId,
}: {
	editingProfileId?: string;
}) {
	const isProfileScope = editingProfileId && editingProfileId !== "default";

	const { data: settings } = useSettings();
	const updateOcrBaseUrl = useUpdateOcrBaseUrl();
	const updateOcrModel = useUpdateOcrModel();
	const updateOcrAuthMode = useUpdateOcrAuthMode();
	const updateOcrPrompt = useUpdateOcrPrompt();
	const updateOcrMaxTokens = useUpdateOcrMaxTokens();
	const updateOcrTemperature = useUpdateOcrTemperature();
	const updateOcrTopP = useUpdateOcrTopP();
	const updateOcrRequestTimeoutMs = useUpdateOcrRequestTimeoutMs();
	const updateOcrContextMaxChars = useUpdateOcrContextMaxChars();
	const updateOcrAutoCaptureTiming = useUpdateOcrAutoCaptureTiming();
	const updateOcrHallucinationProtection =
		useUpdateOcrHallucinationProtection();
	const updateOcrHallucinationThreshold = useUpdateOcrHallucinationThreshold();
	const updateOcrResizeMaxDimension = useUpdateOcrResizeMaxDimension();
	const updateOcrResizeFilter = useUpdateOcrResizeFilter();
	const setOcrApiKey = useSetOcrApiKey();
	const clearOcrApiKey = useClearOcrApiKey();

	const commitInt = (
		raw: string,
		fallback: number,
		commit: (n: number) => void,
	) => {
		const trimmed = raw.trim();
		if (!trimmed) {
			commit(fallback);
			return;
		}
		const parsed = Number(trimmed);
		if (!Number.isFinite(parsed)) return;
		commit(parsed);
	};

	const commitFloat = (
		raw: string,
		fallback: number,
		commit: (n: number) => void,
	) => {
		const trimmed = raw.trim();
		if (!trimmed) {
			commit(fallback);
			return;
		}
		const parsed = Number(trimmed);
		if (!Number.isFinite(parsed)) return;
		commit(parsed);
	};

	const handleOcrApiKeySave = () => {
		const trimmed = ocrApiKeyDraft.trim();
		if (!trimmed) return;
		setOcrApiKey.mutate(trimmed);
	};

	const [ocrBaseUrlDraft, setOcrBaseUrlDraft] = useState(
		settings?.ocr_base_url ?? "",
	);
	const [ocrModelDraft, setOcrModelDraft] = useState(settings?.ocr_model ?? "");
	const [ocrPromptDraft, setOcrPromptDraft] = useState(
		settings?.ocr_prompt ?? "",
	);
	const [ocrMaxTokensDraft, setOcrMaxTokensDraft] = useState(
		String(settings?.ocr_max_tokens ?? 512),
	);
	const [ocrTemperatureDraft, setOcrTemperatureDraft] = useState(
		String(settings?.ocr_temperature ?? 0),
	);
	const [ocrTopPDraft, setOcrTopPDraft] = useState(
		String(settings?.ocr_top_p ?? 1),
	);
	const [ocrTimeoutDraft, setOcrTimeoutDraft] = useState(
		String(settings?.ocr_request_timeout_ms ?? 2000),
	);
	const [ocrMaxCharsDraft, setOcrMaxCharsDraft] = useState(
		String(settings?.ocr_context_max_chars ?? 8000),
	);
	const [ocrApiKeyDraft, setOcrApiKeyDraft] = useState("");
	const ocrApiKeyHydratedRef = useRef(false);

	const { data: storedOcrApiKey } = useQuery({
		queryKey: ["apiKeyValue", "ocr_api_key"],
		queryFn: () => tauriAPI.getApiKey("ocr_api_key"),
		staleTime: 0,
	});

	useEffect(() => {
		setOcrBaseUrlDraft(settings?.ocr_base_url ?? "");
		setOcrModelDraft(settings?.ocr_model ?? "");
		setOcrPromptDraft(settings?.ocr_prompt ?? "");
		setOcrMaxTokensDraft(String(settings?.ocr_max_tokens ?? 512));
		setOcrTemperatureDraft(String(settings?.ocr_temperature ?? 0));
		setOcrTopPDraft(String(settings?.ocr_top_p ?? 1));
		setOcrTimeoutDraft(String(settings?.ocr_request_timeout_ms ?? 2000));
		setOcrMaxCharsDraft(String(settings?.ocr_context_max_chars ?? 8000));
	}, [
		settings?.ocr_base_url,
		settings?.ocr_model,
		settings?.ocr_prompt,
		settings?.ocr_max_tokens,
		settings?.ocr_temperature,
		settings?.ocr_top_p,
		settings?.ocr_request_timeout_ms,
		settings?.ocr_context_max_chars,
	]);

	useEffect(() => {
		if (ocrApiKeyHydratedRef.current) return;
		if (!storedOcrApiKey) return;
		setOcrApiKeyDraft(storedOcrApiKey);
		ocrApiKeyHydratedRef.current = true;
	}, [storedOcrApiKey]);

	const content = (
		<div className="settings-accordion-block" style={{ marginTop: 0 }}>
			<Accordion variant="separated" radius="md">
				<Accordion.Item value="ocr">
					<Accordion.Control>OCR</Accordion.Control>
					<Accordion.Panel>
						<SettingsRow
							label="OCR Base URL"
							description="Base URL for the OCR provider (e.g. http://localhost:8000)."
							right={
								<TextInput
									value={ocrBaseUrlDraft}
									onChange={(e) => setOcrBaseUrlDraft(e.currentTarget.value)}
									onBlur={() =>
										updateOcrBaseUrl.mutate(ocrBaseUrlDraft.trim() || null)
									}
									placeholder="http://localhost:8000"
									styles={{
										input: {
											backgroundColor: "var(--bg-elevated)",
											borderColor: "var(--border-default)",
											color: "var(--text-primary)",
											minWidth: 260,
										},
									}}
								/>
							}
						/>

						<SettingsRow
							label="OCR Model"
							description="Model identifier for OCR (default: lightonai/LightOnOCR-1B-1025)."
							right={
								<TextInput
									value={ocrModelDraft}
									onChange={(e) => setOcrModelDraft(e.currentTarget.value)}
									onBlur={() =>
										updateOcrModel.mutate(ocrModelDraft.trim() || null)
									}
									placeholder="lightonai/LightOnOCR-1B-1025"
									styles={{
										input: {
											backgroundColor: "var(--bg-elevated)",
											borderColor: "var(--border-default)",
											color: "var(--text-primary)",
											minWidth: 260,
										},
									}}
								/>
							}
						/>

						<SettingsRow
							label="OCR Prompt"
							description="Instructions sent along with the screenshot. Plain text output is recommended."
							right={
								<Textarea
									value={ocrPromptDraft}
									onChange={(e) => setOcrPromptDraft(e.currentTarget.value)}
									onBlur={() => updateOcrPrompt.mutate(ocrPromptDraft)}
									autosize
									minRows={3}
									styles={{
										input: {
											backgroundColor: "var(--bg-elevated)",
											borderColor: "var(--border-default)",
											color: "var(--text-primary)",
											minWidth: 260,
										},
									}}
								/>
							}
						/>

						<SettingsRow
							label="OCR Max Tokens"
							description="Maximum number of output tokens to request from the OCR model."
							right={
								<TextInput
									type="number"
									value={ocrMaxTokensDraft}
									onChange={(e) => setOcrMaxTokensDraft(e.currentTarget.value)}
									onBlur={() =>
										commitInt(
											ocrMaxTokensDraft,
											settings?.ocr_max_tokens ?? 512,
											(n) => updateOcrMaxTokens.mutate(n),
										)
									}
									onKeyDown={(e) => {
										if (e.key === "Enter") {
											e.currentTarget.blur();
										}
									}}
									styles={{
										input: {
											backgroundColor: "var(--bg-elevated)",
											borderColor: "var(--border-default)",
											color: "var(--text-primary)",
											minWidth: 140,
										},
									}}
								/>
							}
						/>

						<SettingsRow
							label="OCR Temperature"
							description="Controls randomness. Lower is more deterministic."
							right={
								<TextInput
									type="number"
									value={ocrTemperatureDraft}
									onChange={(e) =>
										setOcrTemperatureDraft(e.currentTarget.value)
									}
									onBlur={() =>
										commitFloat(
											ocrTemperatureDraft,
											settings?.ocr_temperature ?? 0,
											(n) => updateOcrTemperature.mutate(n),
										)
									}
									onKeyDown={(e) => {
										if (e.key === "Enter") {
											e.currentTarget.blur();
										}
									}}
									styles={{
										input: {
											backgroundColor: "var(--bg-elevated)",
											borderColor: "var(--border-default)",
											color: "var(--text-primary)",
											minWidth: 140,
										},
									}}
								/>
							}
						/>

						<SettingsRow
							label="OCR Top P"
							description="Nucleus sampling cutoff. 1.0 disables nucleus sampling."
							right={
								<TextInput
									type="number"
									value={ocrTopPDraft}
									onChange={(e) => setOcrTopPDraft(e.currentTarget.value)}
									onBlur={() =>
										commitFloat(ocrTopPDraft, settings?.ocr_top_p ?? 1, (n) =>
											updateOcrTopP.mutate(n),
										)
									}
									onKeyDown={(e) => {
										if (e.key === "Enter") {
											e.currentTarget.blur();
										}
									}}
									styles={{
										input: {
											backgroundColor: "var(--bg-elevated)",
											borderColor: "var(--border-default)",
											color: "var(--text-primary)",
											minWidth: 140,
										},
									}}
								/>
							}
						/>

						<SettingsRow
							label="OCR Auth Mode"
							description="Choose whether the OCR provider requires an API key."
							right={
								<Select
									data={[
										{ value: "none", label: "None" },
										{ value: "bearer_api_key", label: "API key" },
									]}
									value={settings?.ocr_auth_mode ?? "none"}
									onChange={(value) => {
										if (!value) return;
										updateOcrAuthMode.mutate(
											value === "bearer_api_key" ? "bearer_api_key" : "none",
										);
									}}
									withCheckIcon={false}
									styles={{
										input: {
											backgroundColor: "var(--bg-elevated)",
											borderColor: "var(--border-default)",
											color: "var(--text-primary)",
											minWidth: 200,
										},
									}}
								/>
							}
						/>

						{settings?.ocr_auth_mode === "bearer_api_key" && (
							<SettingsRow
								label="OCR API Key"
								description="Stored securely in your OS credential vault."
								right={
									<Group gap={8} wrap="nowrap">
										<PasswordInput
											value={ocrApiKeyDraft}
											onChange={(e) => setOcrApiKeyDraft(e.currentTarget.value)}
											placeholder="sk-..."
											styles={{
												input: {
													backgroundColor: "var(--bg-elevated)",
													borderColor: "var(--border-default)",
													color: "var(--text-primary)",
													minWidth: 220,
													height: 36,
												},
											}}
											onKeyDown={(e) => {
												if (e.key === "Enter") handleOcrApiKeySave();
											}}
										/>
										<Button
											size="sm"
											color="orange"
											onClick={handleOcrApiKeySave}
											disabled={!ocrApiKeyDraft.trim()}
										>
											Set
										</Button>
										<Button
											size="sm"
											variant="default"
											onClick={() => {
												clearOcrApiKey.mutate();
												setOcrApiKeyDraft("");
											}}
										>
											Clear
										</Button>
									</Group>
								}
							/>
						)}

						<SettingsRow
							label="OCR Timeout"
							description="Maximum time to wait for OCR (milliseconds)."
							right={
								<TextInput
									type="number"
									value={ocrTimeoutDraft}
									onChange={(e) => setOcrTimeoutDraft(e.currentTarget.value)}
									onBlur={() =>
										commitInt(
											ocrTimeoutDraft,
											settings?.ocr_request_timeout_ms ?? 2000,
											(n) => updateOcrRequestTimeoutMs.mutate(n),
										)
									}
									onKeyDown={(e) => {
										if (e.key === "Enter") {
											e.currentTarget.blur();
										}
									}}
									styles={{
										input: {
											backgroundColor: "var(--bg-elevated)",
											borderColor: "var(--border-default)",
											color: "var(--text-primary)",
											minWidth: 140,
										},
									}}
								/>
							}
						/>

						<SettingsRow
							label="OCR Max Chars"
							description="Limit OCR text included in prompts."
							right={
								<TextInput
									type="number"
									value={ocrMaxCharsDraft}
									onChange={(e) => setOcrMaxCharsDraft(e.currentTarget.value)}
									onBlur={() =>
										commitInt(
											ocrMaxCharsDraft,
											settings?.ocr_context_max_chars ?? 8000,
											(n) => updateOcrContextMaxChars.mutate(n),
										)
									}
									onKeyDown={(e) => {
										if (e.key === "Enter") {
											e.currentTarget.blur();
										}
									}}
									styles={{
										input: {
											backgroundColor: "var(--bg-elevated)",
											borderColor: "var(--border-default)",
											color: "var(--text-primary)",
											minWidth: 140,
										},
									}}
								/>
							}
						/>

						<SettingsRow
							label="Auto Capture Timing"
							description="When to capture the screen in Auto mode. 'On start' allows OCR to run in parallel with recording."
							right={
								<Select
									data={[
										{ value: "on_stop", label: "When recording stops" },
										{ value: "on_start", label: "When recording starts" },
									]}
									value={settings?.ocr_auto_capture_timing ?? "on_start"}
									onChange={(value) => {
										if (!value) return;
										updateOcrAutoCaptureTiming.mutate(
											value === "on_start" ? "on_start" : "on_stop",
										);
									}}
									withCheckIcon={false}
									styles={{
										input: {
											backgroundColor: "var(--bg-elevated)",
											borderColor: "var(--border-default)",
											color: "var(--text-primary)",
											minWidth: 200,
										},
									}}
								/>
							}
						/>

						<SettingsRow
							label="Hallucination Threshold"
							description="Variance threshold for image validation. Higher = more permissive, lower = stricter."
							right={
								<NumberInput
									value={settings?.ocr_hallucination_threshold ?? 2500}
									onChange={(value) => {
										const num = typeof value === "number" ? value : 2000;
										updateOcrHallucinationThreshold.mutate(Math.max(0, num));
									}}
									min={0}
									max={50000}
									step={100}
									styles={{
										input: {
											backgroundColor: "var(--bg-elevated)",
											borderColor: "var(--border-default)",
											color: "var(--text-primary)",
											width: 100,
										},
									}}
								/>
							}
						/>

						<SettingsRow
							label="Hallucination Protection"
							description="Skip OCR when the captured image is blank or uniform color to prevent nonsense output."
							right={
								<Switch
									checked={settings?.ocr_hallucination_protection ?? true}
									onChange={(e) =>
										updateOcrHallucinationProtection.mutate(
											e.currentTarget.checked,
										)
									}
								/>
							}
						/>

						<SettingsRow
							label="Resize Max Dimension"
							description="Max width/height for captured images (0 = no resize). Smaller = faster capture."
							right={
								<NumberInput
									value={settings?.ocr_resize_max_dimension ?? 0}
									onChange={(value) => {
										const num = typeof value === "number" ? value : 0;
										updateOcrResizeMaxDimension.mutate(Math.max(0, num));
									}}
									min={0}
									max={4096}
									step={100}
									styles={{
										input: {
											backgroundColor: "var(--bg-elevated)",
											borderColor: "var(--border-default)",
											color: "var(--text-primary)",
											width: 100,
										},
									}}
								/>
							}
						/>

						<SettingsRow
							label="Resize Filter"
							description="Image resize algorithm. Nearest = fastest, Lanczos3 = best quality but very slow."
							right={
								<Select
									data={[
										{ value: "nearest", label: "Nearest (fastest)" },
										{ value: "triangle", label: "Triangle (bilinear)" },
										{ value: "catmullrom", label: "CatmullRom" },
										{ value: "lanczos3", label: "Lanczos3 (slowest)" },
									]}
									value={settings?.ocr_resize_filter ?? "nearest"}
									onChange={(value) => {
										if (
											value === "nearest" ||
											value === "triangle" ||
											value === "catmullrom" ||
											value === "lanczos3"
										) {
											updateOcrResizeFilter.mutate(value);
										}
									}}
									withCheckIcon={false}
									styles={{
										input: {
											backgroundColor: "var(--bg-elevated)",
											borderColor: "var(--border-default)",
											color: "var(--text-primary)",
											minWidth: 180,
										},
									}}
								/>
							}
						/>
					</Accordion.Panel>
				</Accordion.Item>
			</Accordion>
		</div>
	);

	if (isProfileScope) {
		return (
			<Tooltip label={GLOBAL_ONLY_TOOLTIP} withArrow position="top-start">
				<div style={{ opacity: 0.5, cursor: "not-allowed" }}>
					<div style={{ pointerEvents: "none" }}>{content}</div>
				</div>
			</Tooltip>
		);
	}

	return content;
}
