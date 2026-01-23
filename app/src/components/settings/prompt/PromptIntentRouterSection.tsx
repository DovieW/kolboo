import {
	Accordion,
	Button,
	NumberInput,
	Select,
	Switch,
	Text,
	Textarea,
	TextInput,
} from "@mantine/core";
import type {
	AppSettings,
	IntentRouterSettings,
	ModelOption,
	RewritePreset,
} from "../../../lib/tauri";
import { HintSelect } from "../../HintSelect";
import { HintSelectWithDefaultHint } from "../../HintSelectWithDefaultHint";
import { isOpenAiReasoningEffort, normalizeRouter } from "./settingsUtils";
import {
	formatThinkingBudgetShort,
	openAiDefaultReasoningEffortForModel,
	openAiThinkingEffortsForModel,
} from "./useThinkingOptions";

type PromptIntentRouterSectionProps = {
	activeProfileId: string;
	presets: RewritePreset[];
	settings: AppSettings | undefined;
	profileRouter: IntentRouterSettings | null | undefined;
	effectiveRouter: IntentRouterSettings | null;
	routerStrategyValue: "off" | "embeddings" | "llm";
	embeddingProviderValue: string;
	embeddingModels: ModelOption[];
	embeddingModelValue: string | null;
	isCachingRouterEmbeddings: boolean;
	selectDefaultValue: string;
	anthropicThinkingBudgets: readonly number[];
	getEmbeddingModelsForProvider: (provider: string) => ModelOption[];
	getLlmModelOptionsForProvider: (provider: string | null) => ModelOption[];
	saveRouter: (router: IntentRouterSettings | null) => void;
	onCacheRouterEmbeddings: () => void;
};

export function PromptIntentRouterSection({
	activeProfileId,
	presets,
	settings,
	profileRouter,
	effectiveRouter,
	routerStrategyValue,
	embeddingProviderValue,
	embeddingModels,
	embeddingModelValue,
	isCachingRouterEmbeddings,
	selectDefaultValue,
	anthropicThinkingBudgets,
	getEmbeddingModelsForProvider,
	getLlmModelOptionsForProvider,
	saveRouter,
	onCacheRouterEmbeddings,
}: PromptIntentRouterSectionProps) {
	return (
		<Accordion.Item value={`${activeProfileId}-intent-router`}>
			<Accordion.Control>
				<div>
					<p className="settings-label">Intent router</p>
					<p className="settings-description">
						Automatically select a preset based on the transcript.
					</p>
				</div>
			</Accordion.Control>
			<Accordion.Panel>
				<div
					style={{
						display: "flex",
						flexDirection: "column",
						gap: 12,
					}}
				>
					{presets.length === 0 ? (
						<Text size="sm" c="dimmed">
							Add at least one preset to enable routing.
						</Text>
					) : null}

					<div className="settings-row no-divider" style={{ paddingTop: 0 }}>
						<div>
							<p className="settings-label">Router strategy</p>
							<p className="settings-description">
								Off disables routing completely. Embeddings is fast and
								deterministic; LLM can be more flexible but costs more.
							</p>
						</div>
						<Select
							data={[
								{ value: "off", label: "Off" },
								{ value: "embeddings", label: "Embeddings" },
								{ value: "llm", label: "LLM" },
							]}
							value={routerStrategyValue}
							onChange={(value) => {
								if (!value) return;
								if (value === "off") {
									saveRouter({
										enabled: false,
										strategy: "off",
										embedding_provider: null,
										embedding_model: null,
										pick_highest_score: null,
										similarity_threshold: null,
										similarity_margin: null,
										llm_provider: null,
										llm_model: null,
										openai_reasoning_effort: null,
										gemini_thinking_budget: null,
										gemini_thinking_level: null,
										anthropic_thinking_budget: null,
										llm_system_prompt: null,
									});
									return;
								}

								if (value === "embeddings") {
									const provider = "openai";
									const modelOptions = getEmbeddingModelsForProvider(provider);
									const modelValue = modelOptions[0]?.value ?? null;

									saveRouter({
										enabled: true,
										strategy: "embeddings",
										embedding_provider: provider,
										embedding_model: modelValue,
										pick_highest_score:
											effectiveRouter?.pick_highest_score ?? true,
										similarity_threshold:
											effectiveRouter?.similarity_threshold ?? null,
										similarity_margin:
											effectiveRouter?.similarity_margin ?? null,
										llm_provider: null,
										llm_model: null,
										openai_reasoning_effort: null,
										gemini_thinking_budget: null,
										gemini_thinking_level: null,
										anthropic_thinking_budget: null,
										llm_system_prompt: null,
									});
									return;
								}

								const seedProvider =
									settings?.llm_provider ??
									effectiveRouter?.llm_provider ??
									"openai";
								const modelOptions =
									getLlmModelOptionsForProvider(seedProvider);
								const seedModel =
									effectiveRouter?.llm_model ??
									settings?.llm_model ??
									modelOptions[0]?.value ??
									null;

								saveRouter({
									enabled: true,
									strategy: "llm",
									embedding_provider: null,
									embedding_model: null,
									pick_highest_score: null,
									similarity_threshold: null,
									similarity_margin: null,

									llm_provider: seedProvider,
									llm_model: seedModel,
									openai_reasoning_effort:
										settings?.openai_reasoning_effort ?? null,
									gemini_thinking_budget:
										settings?.gemini_thinking_budget ?? null,
									gemini_thinking_level:
										settings?.gemini_thinking_level ?? null,
									anthropic_thinking_budget:
										settings?.anthropic_thinking_budget ?? null,
									llm_system_prompt: null,
								});
							}}
							withCheckIcon={false}
							disabled={presets.length === 0}
							styles={{
								input: {
									backgroundColor: "var(--bg-elevated)",
									borderColor: "var(--border-default)",
									color: "var(--text-primary)",
									minWidth: 200,
								},
							}}
						/>
					</div>

					{routerStrategyValue === "embeddings" ? (
						<>
							<Text size="xs" c="dimmed">
								Uses your{" "}
								{embeddingProviderValue === "cohere" ? "Cohere" : "OpenAI"} API
								key. Configure it in API Keys.
							</Text>

							<div className="settings-row">
								<div>
									<p className="settings-label">Embedding provider</p>
									<p className="settings-description">
										Provider used to embed the transcript and hints.
									</p>
								</div>
								<Select
									data={[
										{ value: "openai", label: "OpenAI" },
										{ value: "cohere", label: "Cohere" },
									]}
									value={embeddingProviderValue}
									onChange={(value) => {
										if (!value) return;
										const models = getEmbeddingModelsForProvider(value);
										const nextModel = models[0]?.value ?? null;
										const next = normalizeRouter(profileRouter);
										const provider =
											value === "openai" || value === "cohere" ? value : null;
										if (!provider) return;
										saveRouter({
											...next,
											enabled: true,
											strategy: "embeddings",
											embedding_provider: provider,
											embedding_model: nextModel,
										});
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
							</div>

							<div className="settings-row">
								<div>
									<p className="settings-label">Embedding model</p>
									<p className="settings-description">
										Model used to embed the transcript and hints.
									</p>
								</div>
								<Select
									data={embeddingModels}
									value={embeddingModelValue}
									onChange={(value) => {
										if (!value) return;
										const next = normalizeRouter(profileRouter);
										const provider =
											embeddingProviderValue === "openai" ||
											embeddingProviderValue === "cohere"
												? embeddingProviderValue
												: null;
										if (!provider) return;
										saveRouter({
											...next,
											enabled: true,
											strategy: "embeddings",
											embedding_provider: provider,
											embedding_model: value,
										});
									}}
									withCheckIcon={false}
									styles={{
										input: {
											backgroundColor: "var(--bg-elevated)",
											borderColor: "var(--border-default)",
											color: "var(--text-primary)",
											minWidth: 240,
										},
									}}
								/>
							</div>

							<div className="settings-row">
								<div>
									<p className="settings-label">Pick highest score</p>
									<p className="settings-description">
										Always selects the candidate with the highest similarity
										score. Disables threshold + margin.
									</p>
								</div>
								<Switch
									checked={Boolean(effectiveRouter?.pick_highest_score)}
									onChange={(e) => {
										const enabled = e.currentTarget.checked;
										const next = normalizeRouter(profileRouter);
										saveRouter({
											...next,
											enabled: true,
											strategy: "embeddings",
											pick_highest_score: enabled,
										});
									}}
									color="gray"
									size="md"
									disabled={presets.length === 0}
								/>
							</div>

							<div className="settings-row">
								<div>
									<p className="settings-label">Store preset embeddings</p>
									<p className="settings-description">
										Precompute and store embeddings for preset hints so routing
										doesn’t re-embed them every run.
									</p>
								</div>
								<Button
									color="gray"
									loading={isCachingRouterEmbeddings}
									disabled={
										isCachingRouterEmbeddings ||
										presets.length === 0 ||
										activeProfileId === "default" ||
										!embeddingModelValue
									}
									onClick={onCacheRouterEmbeddings}
								>
									Store embeddings
								</Button>
							</div>

							<div className="settings-row">
								<div>
									<p className="settings-label">Similarity threshold</p>
									<p className="settings-description">
										Minimum cosine similarity to accept a match.
									</p>
								</div>
								<NumberInput
									value={effectiveRouter?.similarity_threshold ?? 0.78}
									onChange={(value) => {
										if (typeof value !== "number" || Number.isNaN(value)) {
											return;
										}
										const next = normalizeRouter(profileRouter);
										saveRouter({
											...next,
											enabled: true,
											strategy: "embeddings",
											similarity_threshold: value,
										});
									}}
									min={0}
									max={1}
									step={0.01}
									clampBehavior="blur"
									disabled={Boolean(effectiveRouter?.pick_highest_score)}
									styles={{
										input: {
											backgroundColor: "var(--bg-elevated)",
											borderColor: "var(--border-default)",
											color: "var(--text-primary)",
											width: 140,
										},
									}}
								/>
							</div>

							<div className="settings-row no-divider">
								<div>
									<p className="settings-label">Similarity margin</p>
									<p className="settings-description">
										Required gap between the best and second-best preset.
									</p>
								</div>
								<NumberInput
									value={effectiveRouter?.similarity_margin ?? 0.05}
									onChange={(value) => {
										if (typeof value !== "number" || Number.isNaN(value)) {
											return;
										}
										const next = normalizeRouter(profileRouter);
										saveRouter({
											...next,
											enabled: true,
											strategy: "embeddings",
											similarity_margin: value,
										});
									}}
									min={0}
									max={1}
									step={0.01}
									clampBehavior="blur"
									disabled={Boolean(effectiveRouter?.pick_highest_score)}
									styles={{
										input: {
											backgroundColor: "var(--bg-elevated)",
											borderColor: "var(--border-default)",
											color: "var(--text-primary)",
											width: 140,
										},
									}}
								/>
							</div>
						</>
					) : null}

					{routerStrategyValue === "llm"
						? (() => {
								const routerProvider =
									effectiveRouter?.llm_provider ??
									settings?.llm_provider ??
									"openai";
								const modelOptions =
									getLlmModelOptionsForProvider(routerProvider);
								const routerModel =
									effectiveRouter?.llm_model ??
									settings?.llm_model ??
									modelOptions[0]?.value ??
									null;

								const supportsRouterOpenAiThinking =
									routerProvider === "openai" &&
									!!routerModel &&
									(routerModel.startsWith("gpt-5") ||
										routerModel.startsWith("o"));

								const routerOpenAiThinkingOptions =
									!supportsRouterOpenAiThinking || !routerModel
										? []
										: [
												{
													value: selectDefaultValue,
													label: "Default",
												},
												...openAiThinkingEffortsForModel(routerModel).map(
													(v) => ({
														value: v,
														label:
															v === "none"
																? "None"
																: v.charAt(0).toUpperCase() + v.slice(1),
													}),
												),
											];

								const routerOpenAiThinkingDefaultHint = routerModel
									? (settings?.openai_reasoning_effort ??
										openAiDefaultReasoningEffortForModel(routerModel))
									: (settings?.openai_reasoning_effort ?? "medium");

								const supportsRouterGeminiThinkingLevel =
									routerProvider === "gemini" &&
									!!routerModel &&
									routerModel.includes("gemini-3");

								const routerIsGemini3Flash =
									supportsRouterGeminiThinkingLevel &&
									routerModel.includes("gemini-3-flash");

								const routerIsGemini3Pro =
									supportsRouterGeminiThinkingLevel &&
									routerModel.includes("gemini-3-pro");

								const routerGeminiThinkingLevelOptions =
									!supportsRouterGeminiThinkingLevel
										? []
										: routerIsGemini3Flash
											? [
													{
														value: selectDefaultValue,
														label: "Default",
													},
													{ value: "minimal", label: "Minimal" },
													{ value: "low", label: "Low" },
													{ value: "medium", label: "Medium" },
													{ value: "high", label: "High" },
												]
											: [
													{
														value: selectDefaultValue,
														label: "Default",
													},
													{ value: "low", label: "Low" },
													{ value: "high", label: "High" },
												];

								const routerGeminiThinkingLevelDefaultHint =
									settings?.gemini_thinking_level ?? "high";

								const supportsRouterGeminiThinkingBudget =
									routerProvider === "gemini" &&
									!!routerModel &&
									routerModel.includes("gemini-2.5") &&
									!routerModel.includes("flash-lite");

								const routerCanDisableGemini25Thinking =
									supportsRouterGeminiThinkingBudget &&
									routerModel.includes("gemini-2.5-flash") &&
									!routerModel.includes("gemini-2.5-pro");

								const routerIsGemini25Pro =
									supportsRouterGeminiThinkingBudget &&
									routerModel.includes("gemini-2.5-pro");

								const routerGemini25MaxBudget = routerIsGemini25Pro
									? 32768
									: 24576;

								const routerGemini25MinBudget = routerIsGemini25Pro ? 128 : 0;

								const routerGeminiThinkingBudgetOptions: Array<{
									value: string;
									label: string;
								}> = !supportsRouterGeminiThinkingBudget
									? []
									: [
											{ value: selectDefaultValue, label: "Default" },
											{ value: "-1", label: "Dynamic (-1)" },
											...(routerCanDisableGemini25Thinking
												? [{ value: "0", label: "Off (0)" }]
												: []),
											...(routerIsGemini25Pro
												? [
														{
															value: String(routerGemini25MinBudget),
															label: "Minimal (128)",
														},
													]
												: []),
											{ value: "1024", label: "Light (1024)" },
											{ value: "4096", label: "Medium (4096)" },
											{ value: "16384", label: "High (16384)" },
											...(routerGemini25MaxBudget > 16384
												? [
														{
															value: String(routerGemini25MaxBudget),
															label: `Max (${routerGemini25MaxBudget})`,
														},
													]
												: []),
										];

								const routerGeminiThinkingBudgetDefaultHint = (() => {
									const inherited = settings?.gemini_thinking_budget;
									if (inherited == null) return "dynamic";
									if (inherited === 0) return "off";
									if (inherited === -1) return "dynamic";
									return String(inherited);
								})();

								const supportsRouterAnthropicThinkingBudget =
									routerProvider === "anthropic" &&
									!!routerModel &&
									(routerModel.includes("claude-3-7") ||
										routerModel.includes("claude-4") ||
										routerModel.includes("-4-"));

								const routerAnthropicThinkingLevelOptions: Array<{
									value: string;
									label: string;
								}> = !supportsRouterAnthropicThinkingBudget
									? []
									: [
											{ value: selectDefaultValue, label: "Default" },
											{ value: "0", label: "Off" },
											{
												value: String(anthropicThinkingBudgets[0]),
												label: "Low",
											},
											{
												value: String(anthropicThinkingBudgets[1]),
												label: "Medium",
											},
											{
												value: String(anthropicThinkingBudgets[2]),
												label: "High",
											},
											{
												value: String(anthropicThinkingBudgets[3]),
												label: "Max",
											},
										];

								const routerAnthropicThinkingLevelOptionsWithCustom = (() => {
									const vRaw = effectiveRouter?.anthropic_thinking_budget;
									const v =
										typeof vRaw === "number" && Number.isFinite(vRaw)
											? Math.trunc(vRaw)
											: null;
									if (v == null) return routerAnthropicThinkingLevelOptions;

									const asString = String(v);
									const exists = routerAnthropicThinkingLevelOptions.some(
										(o) => o.value === asString,
									);
									if (exists) return routerAnthropicThinkingLevelOptions;

									return [
										...routerAnthropicThinkingLevelOptions,
										{ value: asString, label: `Custom (${v})` },
									];
								})();

								return (
									<>
										<Text size="xs" c="dimmed">
											Configure the provider/model used for routing. The router
											uses structured output (JSON) when the selected
											provider/model supports it.
										</Text>

										<div className="settings-row">
											<div>
												<p className="settings-label">Provider</p>
												<p className="settings-description">
													LLM provider used only for routing.
												</p>
											</div>
											<Select
												data={[
													{ value: "openai", label: "OpenAI" },
													{ value: "gemini", label: "Gemini" },
													{ value: "anthropic", label: "Anthropic" },
													{ value: "groq", label: "Groq" },
													{ value: "fireworks", label: "Fireworks" },
													{ value: "ollama", label: "Ollama" },
												]}
												value={routerProvider}
												onChange={(value) => {
													if (!value) return;
													const next = normalizeRouter(profileRouter);
													const nextModelOptions =
														getLlmModelOptionsForProvider(value);
													const nextModel = nextModelOptions[0]?.value ?? null;
													saveRouter({
														...next,
														enabled: true,
														strategy: "llm",
														llm_provider: value,
														llm_model: nextModel,
													});
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
										</div>

										<div className="settings-row">
											<div>
												<p className="settings-label">Model</p>
												<p className="settings-description">
													Model used for routing decisions.
												</p>
											</div>
											{modelOptions.length > 0 ? (
												<Select
													data={modelOptions}
													value={routerModel}
													onChange={(value) => {
														if (!value) return;
														const next = normalizeRouter(profileRouter);
														saveRouter({
															...next,
															enabled: true,
															strategy: "llm",
															llm_provider: routerProvider,
															llm_model: value,
														});
													}}
													withCheckIcon={false}
													styles={{
														input: {
															backgroundColor: "var(--bg-elevated)",
															borderColor: "var(--border-default)",
															color: "var(--text-primary)",
															minWidth: 240,
														},
													}}
												/>
											) : (
												<TextInput
													value={routerModel ?? ""}
													placeholder="Enter model id"
													onChange={(e) => {
														const value = e.currentTarget.value;
														const next = normalizeRouter(profileRouter);
														saveRouter({
															...next,
															enabled: true,
															strategy: "llm",
															llm_provider: routerProvider,
															llm_model: value.trim().length ? value : null,
														});
													}}
													styles={{
														input: {
															backgroundColor: "var(--bg-elevated)",
															borderColor: "var(--border-default)",
															color: "var(--text-primary)",
															minWidth: 240,
														},
													}}
												/>
											)}
										</div>

										{supportsRouterOpenAiThinking ? (
											<div className="settings-row">
												<div>
													<p className="settings-label">Thinking</p>
													<p className="settings-description">
														Reasoning effort for supported OpenAI models.
													</p>
												</div>
												<HintSelectWithDefaultHint
													data={routerOpenAiThinkingOptions}
													value={
														effectiveRouter?.openai_reasoning_effort ??
														selectDefaultValue
													}
													onChange={(value) => {
														const next = normalizeRouter(profileRouter);
														if (value == null || value === selectDefaultValue) {
															saveRouter({
																...next,
																enabled: true,
																strategy: "llm",
																llm_provider: routerProvider,
																llm_model: routerModel,
																openai_reasoning_effort: null,
															});
															return;
														}

														saveRouter({
															...next,
															enabled: true,
															strategy: "llm",
															llm_provider: routerProvider,
															llm_model: routerModel,
															openai_reasoning_effort: isOpenAiReasoningEffort(
																value,
															)
																? value
																: null,
														});
													}}
													placeholder="Default"
													inputStyle={{
														backgroundColor: "var(--bg-elevated)",
														borderColor: "var(--border-default)",
														color: "var(--text-primary)",
														minWidth: 200,
													}}
													defaultValue={selectDefaultValue}
													defaultHint={routerOpenAiThinkingDefaultHint}
												/>
											</div>
										) : null}

										{supportsRouterGeminiThinkingLevel ? (
											<div className="settings-row">
												<div>
													<p className="settings-label">Thinking level</p>
													<p className="settings-description">
														{routerIsGemini3Pro
															? "Gemini 3 Pro supports low/high (default high)."
															: "Gemini 3 Flash supports minimal/low/medium/high (default high)."}
													</p>
												</div>
												<HintSelectWithDefaultHint
													data={routerGeminiThinkingLevelOptions}
													value={
														effectiveRouter?.gemini_thinking_level ??
														selectDefaultValue
													}
													onChange={(value) => {
														const next = normalizeRouter(profileRouter);
														if (value == null || value === selectDefaultValue) {
															saveRouter({
																...next,
																enabled: true,
																strategy: "llm",
																llm_provider: routerProvider,
																llm_model: routerModel,
																gemini_thinking_level: null,
															});
															return;
														}

														const v =
															value === "minimal" ||
															value === "low" ||
															value === "medium" ||
															value === "high"
																? value
																: null;
														if (v == null) return;

														saveRouter({
															...next,
															enabled: true,
															strategy: "llm",
															llm_provider: routerProvider,
															llm_model: routerModel,
															gemini_thinking_level: v,
														});
													}}
													placeholder="Default"
													inputStyle={{
														backgroundColor: "var(--bg-elevated)",
														borderColor: "var(--border-default)",
														color: "var(--text-primary)",
														minWidth: 200,
													}}
													defaultValue={selectDefaultValue}
													defaultHint={routerGeminiThinkingLevelDefaultHint}
												/>
											</div>
										) : null}

										{supportsRouterGeminiThinkingBudget ? (
											<div className="settings-row">
												<div>
													<p className="settings-label">Thinking budget</p>
													<p className="settings-description">
														Token budget for Gemini 2.5 thinking.
													</p>
												</div>
												<HintSelectWithDefaultHint
													data={routerGeminiThinkingBudgetOptions}
													value={
														effectiveRouter?.gemini_thinking_budget == null
															? selectDefaultValue
															: String(effectiveRouter.gemini_thinking_budget)
													}
													onChange={(value) => {
														const next = normalizeRouter(profileRouter);

														if (value == null || value === selectDefaultValue) {
															saveRouter({
																...next,
																enabled: true,
																strategy: "llm",
																llm_provider: routerProvider,
																llm_model: routerModel,
																gemini_thinking_budget: null,
															});
															return;
														}

														const parsed = Number(value);
														if (!Number.isFinite(parsed)) return;
														const asInt = Math.trunc(parsed);
														saveRouter({
															...next,
															enabled: true,
															strategy: "llm",
															llm_provider: routerProvider,
															llm_model: routerModel,
															gemini_thinking_budget: asInt,
														});
													}}
													placeholder="Default"
													inputStyle={{
														backgroundColor: "var(--bg-elevated)",
														borderColor: "var(--border-default)",
														color: "var(--text-primary)",
														minWidth: 200,
													}}
													defaultValue={selectDefaultValue}
													defaultHint={routerGeminiThinkingBudgetDefaultHint}
												/>
											</div>
										) : null}

										{supportsRouterAnthropicThinkingBudget ? (
											<div className="settings-row">
												<div>
													<p className="settings-label">Thinking</p>
													<p className="settings-description">
														Extended thinking level for Claude models.
													</p>
												</div>
												<HintSelect
													data={routerAnthropicThinkingLevelOptionsWithCustom}
													value={
														effectiveRouter?.anthropic_thinking_budget == null
															? selectDefaultValue
															: String(
																	effectiveRouter.anthropic_thinking_budget,
																)
													}
													onChange={(value) => {
														const next = normalizeRouter(profileRouter);

														if (value == null || value === selectDefaultValue) {
															saveRouter({
																...next,
																enabled: true,
																strategy: "llm",
																llm_provider: routerProvider,
																llm_model: routerModel,
																anthropic_thinking_budget: null,
															});
															return;
														}

														const parsed = Number(value);
														if (!Number.isFinite(parsed)) return;
														const asInt = Math.trunc(parsed);
														saveRouter({
															...next,
															enabled: true,
															strategy: "llm",
															llm_provider: routerProvider,
															llm_model: routerModel,
															anthropic_thinking_budget: asInt,
														});
													}}
													placeholder="Default"
													inputStyle={{
														backgroundColor: "var(--bg-elevated)",
														borderColor: "var(--border-default)",
														color: "var(--text-primary)",
														minWidth: 200,
													}}
													renderSelected={({ option, placeholder }) => {
														if (!option) {
															return (
																<Text size="sm" c="dimmed">
																	{placeholder}
																</Text>
															);
														}

														if (option.value === selectDefaultValue) {
															const inheritedBudget =
																settings?.anthropic_thinking_budget;
															const hint =
																inheritedBudget == null
																	? "off"
																	: formatThinkingBudgetShort(inheritedBudget);

															return (
																<div
																	style={{
																		display: "flex",
																		alignItems: "baseline",
																		gap: 8,
																	}}
																>
																	<span style={{ fontSize: 14 }}>
																		{option.label}
																	</span>
																	<span
																		style={{
																			fontSize: 11,
																			color: "var(--text-muted)",
																			opacity: 0.9,
																			lineHeight: 1,
																		}}
																	>
																		· {hint}
																	</span>
																</div>
															);
														}

														if (option.label.startsWith("Custom")) {
															const n = Number(option.value);
															const suffix = Number.isFinite(n)
																? formatThinkingBudgetShort(n)
																: null;
															return (
																<div
																	style={{
																		display: "flex",
																		alignItems: "baseline",
																		gap: 8,
																	}}
																>
																	<Text size="sm">{option.label}</Text>
																	{suffix && (
																		<Text
																			size="xs"
																			c="dimmed"
																			style={{ lineHeight: 1 }}
																		>
																			{suffix}
																		</Text>
																	)}
																</div>
															);
														}

														return <Text size="sm">{option.label}</Text>;
													}}
													renderOption={({ option }) => {
														if (option.value === selectDefaultValue) {
															const inheritedBudget =
																settings?.anthropic_thinking_budget;
															const hint =
																inheritedBudget == null
																	? "off"
																	: formatThinkingBudgetShort(inheritedBudget);

															return (
																<div
																	style={{
																		display: "flex",
																		alignItems: "baseline",
																		gap: 8,
																	}}
																>
																	<span style={{ fontSize: 14 }}>
																		{option.label}
																	</span>
																	<span
																		style={{
																			fontSize: 11,
																			color: "var(--text-muted)",
																			opacity: 0.9,
																			lineHeight: 1,
																		}}
																	>
																		· {hint}
																	</span>
																</div>
															);
														}

														const n = Number(option.value);
														const suffix = Number.isFinite(n)
															? formatThinkingBudgetShort(n)
															: null;

														return (
															<div
																style={{
																	display: "flex",
																	alignItems: "baseline",
																	gap: 8,
																}}
															>
																<Text size="sm">{option.label}</Text>
																{suffix && (
																	<Text
																		size="xs"
																		c="dimmed"
																		style={{ lineHeight: 1 }}
																	>
																		{suffix}
																	</Text>
																)}
															</div>
														);
													}}
												/>
											</div>
										) : null}

										<div className="settings-row no-divider">
											<div>
												<p className="settings-label">
													System prompt (advanced)
												</p>
												<p className="settings-description">
													Optional override for the router’s system prompt.
													Structured output rules are still enforced.
												</p>
											</div>
											<Textarea
												value={effectiveRouter?.llm_system_prompt ?? ""}
												placeholder="(leave empty to use default router prompt)"
												onChange={(e) => {
													const value = e.currentTarget.value;
													const next = normalizeRouter(profileRouter);
													saveRouter({
														...next,
														enabled: true,
														strategy: "llm",
														llm_provider: routerProvider,
														llm_model: routerModel,
														llm_system_prompt: value.trim().length
															? value
															: null,
													});
												}}
												autosize
												minRows={2}
												styles={{
													input: {
														backgroundColor: "var(--bg-elevated)",
														borderColor: "var(--border-default)",
														color: "var(--text-primary)",
														minWidth: 320,
													},
												}}
											/>
										</div>
									</>
								);
							})()
						: null}
				</div>
			</Accordion.Panel>
		</Accordion.Item>
	);
}
