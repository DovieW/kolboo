import {
	Accordion,
	ActionIcon,
	Box,
	Button,
	Checkbox,
	Divider,
	Group,
	Indicator,
	Kbd,
	NavLink,
	Popover,
	ScrollArea,
	SegmentedControl,
	Select,
	type SelectProps,
	Stack,
	Tabs,
	Text,
	Title,
	Tooltip,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
	BarChart2,
	CircleHelp,
	Cog,
	FileText,
	Filter,
	Home,
	Plus,
	Settings,
} from "lucide-react";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import appPackageJson from "../package.json";
import { HistoryFeed } from "./components/HistoryFeed";
import { Logo } from "./components/Logo";
import { LogsView } from "./components/LogsView";
import {
	AccountSettings,
	ApiKeysSettings,
	AudioSettings,
	DataSettings,
	HotkeySettings,
	NetworkSettings,
	PolicySettings,
	PrivacySettings,
	ProfileConfigModal,
	PromptSettings,
	UiSettings,
} from "./components/settings";
import { SettingsGuideOverlay } from "./components/settings/SettingsGuideOverlay";
import { CostTab, type StatsKindFilter } from "./components/usageStats/CostTab";
import { useModifierKeyForwarder } from "./hooks/useModifierKeyForwarder";
import { applyAccentColor } from "./lib/accentColor";
import { API_KEY_STORE_KEYS } from "./lib/apiKeys";
import {
	readBootAccentColor,
	readBootGuideState,
	setBootGuideState,
} from "./lib/bootStorage";
import { frontendLog } from "./lib/frontendLog";
import {
	DEFAULT_HOLD_HOTKEY,
	DEFAULT_PASTE_LAST_HOTKEY,
	DEFAULT_TOGGLE_HOTKEY,
} from "./lib/hotkeyDefaults";
import { listAllLlmModelKeys, listAllSttModelKeys } from "./lib/modelOptions";
import {
	useSetSettingsGuideState,
	useSettings,
	useSettingsGuideState,
} from "./lib/queries";
import { type CostTimeframe, type HotkeyConfig, tauriAPI } from "./lib/tauri";
import { listenTyped } from "./lib/tauri/events";
import { compareSemver, fetchLatestGithubReleaseVersion } from "./lib/updates";
import "./styles.css";

type View = "home" | "settings" | "logs" | "usage-stats";

function Sidebar({
	activeView,
	onViewChange,
}: {
	activeView: View;
	onViewChange: (view: View) => void;
}) {
	const currentVersion = appPackageJson.version;

	const { data: latestReleaseVersion } = useQuery({
		queryKey: ["latestReleaseVersion", "DovieW", "kolboo"],
		queryFn: () =>
			fetchLatestGithubReleaseVersion({ owner: "DovieW", repo: "kolboo" }),
		staleTime: 6 * 60 * 60 * 1000,
		refetchOnWindowFocus: false,
		retry: false,
	});

	const updateAvailable =
		typeof latestReleaseVersion === "string" &&
		compareSemver(latestReleaseVersion, currentVersion) > 0;

	const releaseUrl = "https://github.com/DovieW/kolboo/releases";

	return (
		<aside className="sidebar">
			<header className="sidebar-header">
				<div className="sidebar-logo">
					<Logo size={32} />
				</div>
			</header>

			<nav className="sidebar-nav">
				<Tooltip label="Home" position="right" withArrow>
					<NavLink
						leftSection={<Home size={20} />}
						active={activeView === "home"}
						onClick={() => onViewChange("home")}
						variant="filled"
						className="sidebar-nav-link"
					/>
				</Tooltip>
				<Tooltip label="Settings" position="right" withArrow>
					<NavLink
						leftSection={<Settings size={20} />}
						active={activeView === "settings"}
						onClick={() => onViewChange("settings")}
						variant="filled"
						className="sidebar-nav-link"
					/>
				</Tooltip>
				<Tooltip label="Stats" position="right" withArrow>
					<NavLink
						leftSection={<BarChart2 size={20} />}
						active={activeView === "usage-stats"}
						onClick={() => onViewChange("usage-stats")}
						variant="filled"
						className="sidebar-nav-link"
					/>
				</Tooltip>
				<Tooltip label="Logs" position="right" withArrow>
					<NavLink
						leftSection={<FileText size={20} />}
						active={activeView === "logs"}
						onClick={() => onViewChange("logs")}
						variant="filled"
						className="sidebar-nav-link"
					/>
				</Tooltip>
			</nav>

			<footer className="sidebar-footer">
				{updateAvailable ? (
					<Tooltip
						label={
							<Text size="xs" fw={700}>
								UPDATE
							</Text>
						}
						withArrow
						position="top"
						offset={6}
						arrowSize={6}
						radius="sm"
						color="red"
						opened
					>
						<a
							className="sidebar-footer-link"
							href={releaseUrl}
							target="_blank"
							rel="noreferrer"
						>
							v{currentVersion}
						</a>
					</Tooltip>
				) : (
					<a
						className="sidebar-footer-link"
						href={releaseUrl}
						target="_blank"
						rel="noreferrer"
					>
						v{currentVersion}
					</a>
				)}
			</footer>
		</aside>
	);
}

function HotkeyDisplay({ config }: { config: HotkeyConfig | null }) {
	if (!config) {
		return <Kbd className="hotkey-placeholder">Unassigned</Kbd>;
	}

	const parts = [
		...config.modifiers.map((m) => m.charAt(0).toUpperCase() + m.slice(1)),
		config.key,
	];

	return (
		<span className="kbd-combo">
			{parts.map((part, index) => (
				<span key={part}>
					<Kbd>{part}</Kbd>
					{index < parts.length - 1 && <span className="kbd-plus">+</span>}
				</span>
			))}
		</span>
	);
}

function _InstructionsCard() {
	const { data: settings } = useSettings();

	const toggleHotkey = settings
		? settings.toggle_hotkey
		: DEFAULT_TOGGLE_HOTKEY;
	const holdHotkey = settings ? settings.hold_hotkey : DEFAULT_HOLD_HOTKEY;
	const pasteLastHotkey = settings
		? settings.paste_last_hotkey
		: DEFAULT_PASTE_LAST_HOTKEY;

	return (
		<div className="instructions-card animate-in">
			<h2 className="instructions-card-title">Dictate with your voice</h2>
			<div className="instructions-methods">
				<div className="instruction-method">
					<span className="instruction-label">Toggle:</span>
					<HotkeyDisplay config={toggleHotkey} />
					<span className="instruction-desc">Press to start/stop</span>
				</div>
				<div className="instruction-method">
					<span className="instruction-label">Hold:</span>
					<HotkeyDisplay config={holdHotkey} />
					<span className="instruction-desc">Hold to record</span>
				</div>
				<div className="instruction-method">
					<span className="instruction-label">Paste:</span>
					<HotkeyDisplay config={pasteLastHotkey} />
					<span className="instruction-desc">Paste last result</span>
				</div>
			</div>
		</div>
	);
}

function HomeView({ onJumpToLog }: { onJumpToLog?: (logId: string) => void }) {
	return (
		<div className="main-content">
			<header className="tv-page-header animate-in">
				<Title order={1} mb={4}>
					Welcome to Kolboo
				</Title>
				<Text c="dimmed" size="sm">
					~-~-~-~-~-~
				</Text>
			</header>

			<div className="main-content-inner">
				{/* <InstructionsCard /> */}
				<HistoryFeed onJumpToLog={onJumpToLog} />
			</div>
		</div>
	);
}

function UsageStatsView() {
	const [activeStatsTab, setActiveStatsTab] = useState<string>("cost");
	const [timeframe, setTimeframe] = useState<CostTimeframe>("30d");
	const [filtersOpened, setFiltersOpened] = useState(false);

	const [statsKind, setStatsKind] = useState<StatsKindFilter>("all");
	const [selectedSttModelKeys, setSelectedSttModelKeys] = useState<string[]>(
		[],
	);
	const [selectedLlmModelKeys, setSelectedLlmModelKeys] = useState<string[]>(
		[],
	);

	// Enabled by default: hide any calls we marked as free-tier.
	const [excludeFreeTier, setExcludeFreeTier] = useState(true);

	const timeframeOptions: Array<{ value: CostTimeframe; label: string }> = [
		{ value: "24h", label: "Last 24 hours" },
		{ value: "7d", label: "Last 7 days" },
		{ value: "30d", label: "Last 30 days" },
		{ value: "90d", label: "Last 90 days" },
		{ value: "all", label: "All time" },
	];

	const sttModelOptions = listAllSttModelKeys();
	const llmModelOptions = listAllLlmModelKeys();

	const hasAnyModelFilter =
		selectedSttModelKeys.length > 0 || selectedLlmModelKeys.length > 0;

	const hasNonDefaultFilters =
		statsKind !== "all" || hasAnyModelFilter || excludeFreeTier !== true;

	return (
		<div className="main-content">
			<header className="tv-page-header animate-in">
				<Group justify="space-between" align="center" wrap="wrap">
					<Title order={1} mb={0}>
						Stats
					</Title>

					<Group gap={8} align="center" wrap="nowrap">
						<Popover
							opened={filtersOpened}
							onChange={setFiltersOpened}
							position="bottom-start"
							shadow="lg"
							radius="md"
						>
							<Popover.Target>
								<Indicator
									disabled={!hasNonDefaultFilters}
									size={8}
									offset={3}
									position="top-end"
									color="orange"
								>
									<ActionIcon
										variant="default"
										size={36}
										onClick={() => setFiltersOpened((v) => !v)}
										title="Filters"
										aria-label="Filters"
										styles={{
											root: {
												backgroundColor: "var(--bg-elevated)",
												borderColor: "var(--border-default)",
											},
										}}
									>
										<Filter size={16} />
									</ActionIcon>
								</Indicator>
							</Popover.Target>

							<Popover.Dropdown
								p={0}
								w={360}
								styles={{
									dropdown: {
										backgroundColor: "var(--bg-elevated)",
										borderColor: "var(--border-default)",
										color: "var(--text-primary)",
									},
								}}
							>
								<Group
									justify="space-between"
									align="center"
									gap={8}
									px="xs"
									py={10}
									wrap="nowrap"
									style={{ minHeight: 32 }}
								>
									<Text size="xs" fw={700}>
										Filters
									</Text>
									{hasNonDefaultFilters ? (
										<Button
											variant="subtle"
											size="compact-xs"
											color="gray"
											onClick={() => {
												setStatsKind("all");
												setSelectedSttModelKeys([]);
												setSelectedLlmModelKeys([]);
												setExcludeFreeTier(true);
											}}
											styles={{ root: { height: 20, padding: "0 6px" } }}
										>
											Reset
										</Button>
									) : (
										// Keep header height stable when Reset is hidden
										<Box w={44} />
									)}
								</Group>

								<Divider color="var(--border-default)" />

								<Box p="xs">
									<SegmentedControl
										value={statsKind}
										onChange={(value) => setStatsKind(value as StatsKindFilter)}
										data={[
											{ value: "all", label: "All" },
											{ value: "stt", label: "STT" },
											{ value: "llm", label: "LLM" },
										]}
										size="xs"
										fullWidth
									/>
								</Box>

								<Box px="xs" pb={10}>
									<Checkbox
										label={<Text size="xs">Exclude free tier</Text>}
										size="xs"
										checked={excludeFreeTier}
										onChange={(e) =>
											setExcludeFreeTier(e.currentTarget.checked)
										}
										styles={{
											body: { alignItems: "center" },
											label: {
												color: "var(--text-primary)",
												paddingLeft: 6,
											},
										}}
									/>
								</Box>

								<Divider color="var(--border-default)" />

								<Box px="xs" py={8}>
									<Accordion
										multiple
										defaultValue={[]}
										variant="separated"
										radius="md"
										chevronPosition="left"
										styles={{
											item: {
												backgroundColor: "transparent",
												border: "1px solid var(--border-default)",
												overflow: "hidden",
											},
											control: {
												backgroundColor: "transparent",
												padding: "6px 10px",
											},
											chevron: {
												color: "var(--text-muted)",
											},
											panel: {
												padding: "0 10px 8px 10px",
											},
										}}
									>
										<Accordion.Item value="stt_models">
											<Accordion.Control>
												<Group justify="space-between" wrap="nowrap" w="100%">
													<Text size="xs" fw={600}>
														STT models
													</Text>
													{selectedSttModelKeys.length > 0 ? (
														<Button
															component="span"
															role="button"
															tabIndex={0}
															variant="subtle"
															size="compact-xs"
															color="gray"
															onClick={(e) => {
																e.preventDefault();
																e.stopPropagation();
																setSelectedSttModelKeys([]);
															}}
															onKeyDown={(e) => {
																if (e.key !== "Enter" && e.key !== " ") return;
																e.preventDefault();
																e.stopPropagation();
																setSelectedSttModelKeys([]);
															}}
															styles={{
																root: { height: 20, padding: "0 6px" },
															}}
														>
															Reset
														</Button>
													) : null}
												</Group>
											</Accordion.Control>
											<Accordion.Panel>
												{sttModelOptions.length === 0 ? (
													<Text c="dimmed" size="xs">
														No models available.
													</Text>
												) : (
													<ScrollArea.Autosize
														mah={180}
														type="auto"
														offsetScrollbars
													>
														<Checkbox.Group
															value={selectedSttModelKeys}
															onChange={(next) => setSelectedSttModelKeys(next)}
														>
															<Stack gap={6}>
																{sttModelOptions.map((opt) => (
																	<Checkbox
																		key={opt.key}
																		value={opt.key}
																		size="xs"
																		label={<Text size="xs">{opt.label}</Text>}
																		styles={{
																			label: { width: "100%" },
																			body: { alignItems: "center" },
																		}}
																	/>
																))}
															</Stack>
														</Checkbox.Group>
													</ScrollArea.Autosize>
												)}
											</Accordion.Panel>
										</Accordion.Item>

										<Accordion.Item value="llm_models">
											<Accordion.Control>
												<Group justify="space-between" wrap="nowrap" w="100%">
													<Text size="xs" fw={600}>
														LLM models
													</Text>
													{selectedLlmModelKeys.length > 0 ? (
														<Button
															component="span"
															role="button"
															tabIndex={0}
															variant="subtle"
															size="compact-xs"
															color="gray"
															onClick={(e) => {
																e.preventDefault();
																e.stopPropagation();
																setSelectedLlmModelKeys([]);
															}}
															onKeyDown={(e) => {
																if (e.key !== "Enter" && e.key !== " ") return;
																e.preventDefault();
																e.stopPropagation();
																setSelectedLlmModelKeys([]);
															}}
															styles={{
																root: { height: 20, padding: "0 6px" },
															}}
														>
															Reset
														</Button>
													) : null}
												</Group>
											</Accordion.Control>
											<Accordion.Panel>
												{llmModelOptions.length === 0 ? (
													<Text c="dimmed" size="xs">
														No models available.
													</Text>
												) : (
													<ScrollArea.Autosize
														mah={180}
														type="auto"
														offsetScrollbars
													>
														<Checkbox.Group
															value={selectedLlmModelKeys}
															onChange={(next) => setSelectedLlmModelKeys(next)}
														>
															<Stack gap={6}>
																{llmModelOptions.map((opt) => (
																	<Checkbox
																		key={opt.key}
																		value={opt.key}
																		size="xs"
																		label={<Text size="xs">{opt.label}</Text>}
																		styles={{
																			label: { width: "100%" },
																			body: { alignItems: "center" },
																		}}
																	/>
																))}
															</Stack>
														</Checkbox.Group>
													</ScrollArea.Autosize>
												)}
											</Accordion.Panel>
										</Accordion.Item>
									</Accordion>
								</Box>
							</Popover.Dropdown>
						</Popover>

						<Select
							value={timeframe}
							onChange={(value) => {
								const next = (value ?? "30d") as CostTimeframe;
								setTimeframe(next);
							}}
							data={timeframeOptions}
							renderOption={({ option }) => option.label}
							allowDeselect={false}
							searchable={false}
							w={220}
							styles={{
								input: {
									backgroundColor: "var(--bg-elevated)",
									borderColor: "var(--border-default)",
									color: "var(--text-primary)",
								},
								dropdown: {
									backgroundColor: "var(--bg-elevated)",
									borderColor: "var(--border-default)",
								},
								option: {
									color: "var(--text-primary)",
								},
							}}
						/>
					</Group>
				</Group>
			</header>

			<div className="main-content-inner">
				<Tabs
					value={activeStatsTab}
					onChange={(value) => {
						if (!value) return;
						setActiveStatsTab(value);
					}}
					keepMounted={false}
				>
					<Tabs.List>
						<Tabs.Tab value="cost">Cost</Tabs.Tab>
					</Tabs.List>

					<Tabs.Panel value="cost" pt="md">
						<CostTab
							timeframe={timeframe}
							kind={statsKind}
							sttModelKeys={selectedSttModelKeys}
							llmModelKeys={selectedLlmModelKeys}
							excludeFreeTier={excludeFreeTier}
						/>
					</Tabs.Panel>
				</Tabs>
			</div>
		</div>
	);
}

function _SettingsView() {
	const { data: settings } = useSettings();
	const profiles = settings?.rewrite_program_prompt_profiles ?? [];
	const [editingProfileId, setEditingProfileId] = useState<string>("default");
	const [programsModalOpen, setProgramsModalOpen] = useState(false);
	const [autoCreateProfileOnOpen, setAutoCreateProfileOnOpen] = useState(false);

	const { data: hasAnyApiKey } = useQuery({
		queryKey: ["hasAnyApiKey"],
		queryFn: async () => {
			try {
				const results = await Promise.all(
					API_KEY_STORE_KEYS.map((key) => tauriAPI.hasApiKey(key)),
				);
				return results.some(Boolean);
			} catch {
				// If we can't determine key status, don't block users by forcing the API Keys tab.
				return true;
			}
		},
	});

	const [activeSettingsTab, setActiveSettingsTab] = useState<string>("ai");
	const [hasUserSelectedTab, setHasUserSelectedTab] = useState(false);

	useEffect(() => {
		if (hasUserSelectedTab) return;
		if (hasAnyApiKey === undefined) return;
		setActiveSettingsTab(hasAnyApiKey ? "ai" : "api-keys");
	}, [hasAnyApiKey, hasUserSelectedTab]);

	useEffect(() => {
		// Consume the one-shot flag as soon as the modal is opened.
		if (programsModalOpen && autoCreateProfileOnOpen) {
			setAutoCreateProfileOnOpen(false);
		}
	}, [programsModalOpen, autoCreateProfileOnOpen]);

	useEffect(() => {
		if (editingProfileId === "default") return;
		if (!profiles.some((p) => p.id === editingProfileId)) {
			setEditingProfileId("default");
		}
	}, [editingProfileId, profiles]);

	const editingOptions = [
		// Keep an explicit Default entry at the top for UX.
		// If the backend has migrated Default into a real persisted profile (id="default"),
		// avoid duplicating it in the options list.
		{ value: "default", label: "Default" },
		...profiles
			.filter((p) => p.id !== "default")
			.map((p) => ({
				value: p.id,
				label: p.name.trim() ? p.name.trim() : p.id,
			})),
	];

	return (
		<div className="main-content">
			<header
				className="tv-page-header animate-in"
				style={{
					display: "flex",
					alignItems: "flex-end",
					justifyContent: "space-between",
					gap: 16,
					flexWrap: "wrap",
				}}
			>
				<div>
					<Title order={1} mb={4}>
						Settings
					</Title>
					<Text c="dimmed" size="sm">
						Configure your preferences
					</Text>
				</div>

				<div
					style={{
						display: "flex",
						alignItems: "center",
						gap: 10,
					}}
				>
					<div
						style={{
							display: "flex",
							alignItems: "center",
							gap: 6,
						}}
					>
						<Tooltip label="New profile" withArrow>
							<ActionIcon
								variant="subtle"
								color="orange"
								size="sm"
								aria-label="New profile"
								onClick={() => {
									setAutoCreateProfileOnOpen(true);
									setProgramsModalOpen(true);
								}}
							>
								<Plus size={14} />
							</ActionIcon>
						</Tooltip>

						<Select
							data={editingOptions}
							value={editingProfileId}
							onChange={(v) => setEditingProfileId(v ?? "default")}
							withCheckIcon={false}
							size="xs"
							styles={{
								input: {
									backgroundColor: "transparent",
									border: "1px solid var(--border-default)",
									borderRadius: 6,
									color: "var(--text-primary)",
									minWidth: 140,
									paddingLeft: 8,
									paddingRight: 4,
								},
								dropdown: {
									backgroundColor: "var(--bg-elevated)",
									borderColor: "var(--border-default)",
								},
							}}
						/>

						<Tooltip
							label={
								editingProfileId === "default"
									? "Select a none-default profile to configure programs"
									: "Profile config"
							}
							withArrow
						>
							<ActionIcon
								variant="subtle"
								color="orange"
								size="sm"
								aria-label="Profile config"
								onClick={() => setProgramsModalOpen(true)}
								disabled={editingProfileId === "default"}
							>
								<Cog size={14} />
							</ActionIcon>
						</Tooltip>
					</div>
				</div>
			</header>

			<div className="main-content-inner">
				<ProfileConfigModal
					opened={programsModalOpen}
					onClose={() => {
						setProgramsModalOpen(false);
						setAutoCreateProfileOnOpen(false);
					}}
					editingProfileId={editingProfileId}
					onEditingProfileChange={setEditingProfileId}
					autoCreateProfile={autoCreateProfileOnOpen}
				/>

				<Tabs
					value={activeSettingsTab}
					onChange={(value) => {
						if (!value) return;
						setHasUserSelectedTab(true);
						setActiveSettingsTab(value);
					}}
					classNames={{ root: "settings-tabs" }}
					keepMounted={false}
				>
					<Tabs.List>
						<Tabs.Tab value="account">Account</Tabs.Tab>
						<Tabs.Tab value="ai">AI</Tabs.Tab>
						<Tabs.Tab value="ui">UI</Tabs.Tab>
						<Tabs.Tab value="audio">Audio</Tabs.Tab>
						<Tabs.Tab value="hotkeys">Hotkeys</Tabs.Tab>
						<Tabs.Tab value="api-keys">Providers</Tabs.Tab>
						<Tabs.Tab value="data">Data</Tabs.Tab>
						<Tabs.Tab value="network">Network</Tabs.Tab>
						<Tabs.Tab value="privacy">Privacy</Tabs.Tab>
						<Tabs.Tab value="policy">Policy</Tabs.Tab>
					</Tabs.List>

					<Tabs.Panel value="account" pt="md">
						<div className="settings-card">
							<AccountSettings />
						</div>
					</Tabs.Panel>

					<Tabs.Panel value="ai" pt="md">
						<div className="settings-card">
							<PromptSettings editingProfileId={editingProfileId} />
						</div>
					</Tabs.Panel>

					<Tabs.Panel value="ui" pt="md">
						<div className="settings-card">
							<UiSettings editingProfileId={editingProfileId} />
						</div>
					</Tabs.Panel>

					<Tabs.Panel value="audio" pt="md">
						<div className="settings-card">
							<AudioSettings editingProfileId={editingProfileId} />
						</div>
					</Tabs.Panel>

					<Tabs.Panel value="hotkeys" pt="md">
						<div className="settings-card">
							<HotkeySettings editingProfileId={editingProfileId} />
						</div>
					</Tabs.Panel>

					<Tabs.Panel value="api-keys" pt="md">
						<div className="settings-card">
							<ApiKeysSettings editingProfileId={editingProfileId} />
						</div>
					</Tabs.Panel>

					<Tabs.Panel value="data" pt="md">
						<div className="settings-card">
							<DataSettings editingProfileId={editingProfileId} />
						</div>
					</Tabs.Panel>

					<Tabs.Panel value="network" pt="md">
						<div className="settings-card">
							<NetworkSettings editingProfileId={editingProfileId} />
						</div>
					</Tabs.Panel>

					<Tabs.Panel value="privacy" pt="md">
						<div className="settings-card">
							<PrivacySettings
								onNavigateToTab={(tab) => {
									setHasUserSelectedTab(true);
									setActiveSettingsTab(tab);
								}}
							/>
						</div>
					</Tabs.Panel>

					<Tabs.Panel value="policy" pt="md">
						<div className="settings-card">
							<PolicySettings />
						</div>
					</Tabs.Panel>
				</Tabs>
			</div>
		</div>
	);
}

// Active settings view entrypoint; update UI tweaks here (not _SettingsView).
function SettingsViewWithGuideLauncher({
	onRunSetupGuide,
}: {
	onRunSetupGuide: () => void;
}) {
	const { data: settings } = useSettings();
	const profiles = settings?.rewrite_program_prompt_profiles ?? [];
	const [editingProfileId, setEditingProfileId] = useState<string>("default");
	const [programsModalOpen, setProgramsModalOpen] = useState(false);
	const [autoCreateProfileOnOpen, setAutoCreateProfileOnOpen] = useState(false);
	const selectedProfileDisabled =
		profiles.find((p) => p.id === editingProfileId)?.disabled ?? false;

	const { data: hasAnyApiKey } = useQuery({
		queryKey: ["hasAnyApiKey"],
		queryFn: async () => {
			try {
				const results = await Promise.all(
					API_KEY_STORE_KEYS.map((key) => tauriAPI.hasApiKey(key)),
				);
				return results.some(Boolean);
			} catch {
				// If we can't determine key status, don't block users by forcing the API Keys tab.
				return true;
			}
		},
	});

	const [activeSettingsTab, setActiveSettingsTab] = useState<string>("ai");
	const [hasUserSelectedTab, setHasUserSelectedTab] = useState(false);

	useEffect(() => {
		if (hasUserSelectedTab) return;
		if (hasAnyApiKey === undefined) return;
		setActiveSettingsTab(hasAnyApiKey ? "ai" : "api-keys");
	}, [hasAnyApiKey, hasUserSelectedTab]);

	useEffect(() => {
		// Consume the one-shot flag as soon as the modal is opened.
		if (programsModalOpen && autoCreateProfileOnOpen) {
			setAutoCreateProfileOnOpen(false);
		}
	}, [programsModalOpen, autoCreateProfileOnOpen]);

	useEffect(() => {
		if (editingProfileId === "default") return;
		if (!profiles.some((p) => p.id === editingProfileId)) {
			setEditingProfileId("default");
		}
	}, [editingProfileId, profiles]);

	type EditingOption = {
		value: string;
		label: string;
		isDisabledProfile?: boolean;
	};

	const editingOptions: EditingOption[] = [
		// Keep an explicit Default entry at the top for UX.
		// If the backend has migrated Default into a real persisted profile (id="default"),
		// avoid duplicating it in the options list.
		{ value: "default", label: "Default" },
		...profiles
			.filter((p) => p.id !== "default")
			.map((p) => ({
				value: p.id,
				label: p.name.trim() ? p.name.trim() : p.id,
				isDisabledProfile: p.disabled ?? false,
			})),
	];

	const renderEditingOption: SelectProps["renderOption"] = ({ option }) => {
		const isDisabledProfile = (option as EditingOption).isDisabledProfile;

		return (
			<div
				style={{
					color: isDisabledProfile
						? "var(--text-secondary)"
						: "var(--text-primary)",
					textDecoration: isDisabledProfile ? "line-through" : "none",
				}}
			>
				{option.label}
			</div>
		);
	};

	return (
		<div className="main-content">
			<header
				className="tv-page-header animate-in"
				style={{
					display: "flex",
					alignItems: "flex-end",
					justifyContent: "space-between",
					gap: 16,
					flexWrap: "wrap",
				}}
			>
				<div>
					<Title order={1} mb={4}>
						Settings
					</Title>
					<Text c="dimmed" size="sm">
						Configure your preferences
					</Text>
				</div>

				<div
					style={{
						display: "flex",
						alignItems: "center",
						gap: 10,
					}}
				>
					<div
						style={{
							display: "flex",
							alignItems: "center",
							gap: 6,
						}}
					>
						<Tooltip label="Run setup guide" withArrow>
							<ActionIcon
								variant="subtle"
								color="orange"
								size="sm"
								aria-label="Run setup guide"
								onClick={onRunSetupGuide}
							>
								<CircleHelp size={14} />
							</ActionIcon>
						</Tooltip>

						<Tooltip label="New profile" withArrow>
							<ActionIcon
								variant="subtle"
								color="orange"
								size="sm"
								aria-label="New profile"
								onClick={() => {
									setAutoCreateProfileOnOpen(true);
									setProgramsModalOpen(true);
								}}
							>
								<Plus size={14} />
							</ActionIcon>
						</Tooltip>

						<Select
							data={editingOptions}
							value={editingProfileId}
							onChange={(v) => setEditingProfileId(v ?? "default")}
							renderOption={renderEditingOption}
							withCheckIcon={false}
							size="xs"
							styles={{
								input: {
									backgroundColor: "transparent",
									border: "1px solid var(--border-default)",
									borderRadius: 6,
									color: selectedProfileDisabled
										? "var(--text-secondary)"
										: "var(--text-primary)",
									textDecoration: selectedProfileDisabled
										? "line-through"
										: "none",
									minWidth: 140,
									paddingLeft: 8,
									paddingRight: 4,
								},
								dropdown: {
									backgroundColor: "var(--bg-elevated)",
									borderColor: "var(--border-default)",
								},
							}}
						/>

						<Tooltip
							label={
								editingProfileId === "default"
									? "Select a none-default profile to configure programs"
									: "Profile config"
							}
							withArrow
						>
							<ActionIcon
								variant="subtle"
								color="orange"
								size="sm"
								aria-label="Profile config"
								onClick={() => setProgramsModalOpen(true)}
								disabled={editingProfileId === "default"}
							>
								<Cog size={14} />
							</ActionIcon>
						</Tooltip>
					</div>
				</div>
			</header>

			<div className="main-content-inner">
				<ProfileConfigModal
					opened={programsModalOpen}
					onClose={() => {
						setProgramsModalOpen(false);
						setAutoCreateProfileOnOpen(false);
					}}
					editingProfileId={editingProfileId}
					onEditingProfileChange={setEditingProfileId}
					autoCreateProfile={autoCreateProfileOnOpen}
				/>

				<Tabs
					value={activeSettingsTab}
					onChange={(value) => {
						if (!value) return;
						setHasUserSelectedTab(true);
						setActiveSettingsTab(value);
					}}
					classNames={{ root: "settings-tabs" }}
					keepMounted={false}
				>
					<Tabs.List>
						<Tabs.Tab value="account">Account</Tabs.Tab>
						<Tabs.Tab value="ai">AI</Tabs.Tab>
						<Tabs.Tab value="ui">UI</Tabs.Tab>
						<Tabs.Tab value="audio">Audio</Tabs.Tab>
						<Tabs.Tab value="hotkeys">Hotkeys</Tabs.Tab>
						<Tabs.Tab value="api-keys">Providers</Tabs.Tab>
						<Tabs.Tab value="data">Data</Tabs.Tab>
						<Tabs.Tab value="network">Network</Tabs.Tab>
						<Tabs.Tab value="privacy">Privacy</Tabs.Tab>
						<Tabs.Tab value="policy">Policy</Tabs.Tab>
					</Tabs.List>

					<Tabs.Panel value="account" pt="md">
						<div className="settings-card">
							<AccountSettings />
						</div>
					</Tabs.Panel>

					<Tabs.Panel value="ai" pt="md">
						<div className="settings-card">
							<PromptSettings editingProfileId={editingProfileId} />
						</div>
					</Tabs.Panel>

					<Tabs.Panel value="ui" pt="md">
						<div className="settings-card">
							<UiSettings editingProfileId={editingProfileId} />
						</div>
					</Tabs.Panel>

					<Tabs.Panel value="audio" pt="md">
						<div className="settings-card">
							<AudioSettings editingProfileId={editingProfileId} />
						</div>
					</Tabs.Panel>

					<Tabs.Panel value="hotkeys" pt="md">
						<div className="settings-card">
							<HotkeySettings editingProfileId={editingProfileId} />
						</div>
					</Tabs.Panel>

					<Tabs.Panel value="api-keys" pt="md">
						<div className="settings-card">
							<ApiKeysSettings editingProfileId={editingProfileId} />
						</div>
					</Tabs.Panel>

					<Tabs.Panel value="data" pt="md">
						<div className="settings-card">
							<DataSettings editingProfileId={editingProfileId} />
						</div>
					</Tabs.Panel>

					<Tabs.Panel value="network" pt="md">
						<div className="settings-card">
							<NetworkSettings editingProfileId={editingProfileId} />
						</div>
					</Tabs.Panel>

					<Tabs.Panel value="privacy" pt="md">
						<div className="settings-card">
							<PrivacySettings
								onNavigateToTab={(tab) => {
									setHasUserSelectedTab(true);
									setActiveSettingsTab(tab);
								}}
							/>
						</div>
					</Tabs.Panel>

					<Tabs.Panel value="policy" pt="md">
						<div className="settings-card">
							<PolicySettings />
						</div>
					</Tabs.Panel>
				</Tabs>
			</div>
		</div>
	);
}

function AccentColorSync() {
	const { data: settings } = useSettings();

	// Read once so Ctrl+R can apply the user's accent immediately, without waiting
	// for the async Tauri store to hydrate.
	const bootAccent = useMemo(() => readBootAccentColor(), []);

	// Use layout effect so this runs before paint (avoids a one-frame accent flash).
	useLayoutEffect(() => {
		const effectiveAccent = settings ? settings.accent_color : bootAccent;
		applyAccentColor(effectiveAccent);
	}, [bootAccent, settings]);

	return null;
}

export default function App() {
	// Forward modifier-only key events (like AltRight) to the backend.
	// WebView2 intercepts these before our keyboard hook sees them.
	useModifierKeyForwarder();

	const queryClient = useQueryClient();

	const bootGuideState = readBootGuideState();
	const bootGuideKnown = bootGuideState !== null;
	const bootShouldAutoOpenGuide = bootGuideState === "pending";

	frontendLog.info(
		"boot",
		`guideState=${bootGuideState} known=${bootGuideKnown} autoOpen=${bootShouldAutoOpenGuide}`,
	);

	// Safety valve: on some fresh installs, the first attempt to read the Tauri store
	// (via plugin-store) can error or never resolve until the webview is reloaded.
	// We should never show an infinite blank/black window; after a short grace
	// period, fall back to opening the setup guide.
	const [bootGuideFallbackActivated, setBootGuideFallbackActivated] =
		useState(false);

	const [activeView, setActiveView] = useState<View>(() =>
		bootShouldAutoOpenGuide ? "settings" : "home",
	);
	const [logsJumpToId, setLogsJumpToId] = useState<string | null>(null);
	const [settingsGuideOpen, setSettingsGuideOpen] = useState<boolean>(
		() => bootShouldAutoOpenGuide,
	);
	const lastSingleInstanceToastAt = useRef(0);

	const guideQuery = useSettingsGuideState();
	const guideState = guideQuery.data;
	const setGuideState = useSetSettingsGuideState();

	// Keep the cost summary cache in sync even when the Stats view isn't mounted.
	useEffect(() => {
		let unlisten: (() => void) | null = null;

		tauriAPI
			.onStatsChanged(() => {
				queryClient.invalidateQueries({ queryKey: ["costSummary"] });
				queryClient.invalidateQueries({ queryKey: ["costByProvider"] });
			})
			.then((fn) => {
				unlisten = fn;
			})
			.catch((e) => {
				console.warn("Failed to subscribe to stats-changed:", e);
			});

		return () => {
			try {
				unlisten?.();
			} catch {
				// ignore
			}
		};
	}, [queryClient]);

	useEffect(() => {
		let unlisten: (() => void) | null = null;

		tauriAPI
			.onTranscriptCopiedToClipboard(() => {
				notifications.show({
					title: "Copied to clipboard",
					message:
						"Transcript was copied because the app couldn't safely insert it.",
					color: "orange",
				});
			})
			.then((fn) => {
				unlisten = fn;
			})
			.catch((e) => {
				console.warn("Failed to subscribe to clipboard fallback:", e);
			});

		return () => {
			try {
				unlisten?.();
			} catch {
				// ignore
			}
		};
	}, []);

	useEffect(() => {
		let unlisten: (() => void) | null = null;

		listenTyped("single-instance-activated", () => {
			const now = Date.now();
			if (now - lastSingleInstanceToastAt.current < 1500) {
				return;
			}
			lastSingleInstanceToastAt.current = now;
			notifications.show({
				title: "Already running",
				message: "Kolboo is already running.",
				color: "blue",
			});
		})
			.then((fn) => {
				unlisten = fn;
			})
			.catch((e) => {
				console.warn("Failed to subscribe to single-instance-activated:", e);
			});

		return () => {
			try {
				unlisten?.();
			} catch {
				// ignore
			}
		};
	}, []);

	useEffect(() => {
		if (bootGuideKnown) return;
		if (guideState !== undefined) return;
		if (guideQuery.isError) return;
		if (bootGuideFallbackActivated) return;

		const t = window.setTimeout(() => {
			frontendLog.warn(
				"boot",
				"Boot guide fallback activated (Tauri store read timed out or failed)",
			);
			setBootGuideFallbackActivated(true);
			// Also seed localStorage so subsequent reloads / first-paint logic can
			// immediately decide to open the guide.
			setBootGuideState("pending");
		}, 1200);

		return () => {
			window.clearTimeout(t);
		};
	}, [
		bootGuideFallbackActivated,
		bootGuideKnown,
		guideQuery.isError,
		guideState,
	]);

	useEffect(() => {
		// If the guide state failed to load (or timed out), treat it like a first-run
		// and open the setup guide instead of leaving the user on a blank page.
		if (bootGuideKnown) return;
		if (guideState !== undefined) return;
		if (!guideQuery.isError && !bootGuideFallbackActivated) return;

		setActiveView("settings");
		setSettingsGuideOpen(true);
	}, [
		bootGuideFallbackActivated,
		bootGuideKnown,
		guideQuery.isError,
		guideState,
	]);

	useEffect(() => {
		if (guideState === "pending") {
			setActiveView("settings");
			setSettingsGuideOpen(true);
		}
	}, [guideState]);

	// If we don't have a boot hint yet (first ever run, or storage was cleared),
	// avoid rendering the Home view for a moment before the guide state arrives.
	// IMPORTANT: do not early-return before hooks/effects are declared.
	const shouldShowBootSplash =
		!bootGuideKnown &&
		guideState === undefined &&
		!guideQuery.isError &&
		!bootGuideFallbackActivated;

	const renderView = () => {
		switch (activeView) {
			case "home":
				return (
					<HomeView
						onJumpToLog={(logId) => {
							setLogsJumpToId(logId);
							setActiveView("logs");
						}}
					/>
				);
			case "settings":
				return (
					<SettingsViewWithGuideLauncher
						onRunSetupGuide={() => {
							setSettingsGuideOpen(true);
						}}
					/>
				);
			case "logs":
				return (
					<div className="main-content">
						<LogsView
							jumpToLogId={logsJumpToId}
							onJumpHandled={() => setLogsJumpToId(null)}
						/>
					</div>
				);
			case "usage-stats":
				return <UsageStatsView />;
			default:
				return (
					<HomeView
						onJumpToLog={(logId) => {
							setLogsJumpToId(logId);
							setActiveView("logs");
						}}
					/>
				);
		}
	};

	if (shouldShowBootSplash) {
		return (
			<div
				style={{
					position: "fixed",
					inset: 0,
					background: "#0b0d10",
				}}
			/>
		);
	}

	return (
		<div className="app-layout">
			<AccentColorSync />
			<Sidebar
				activeView={activeView}
				onViewChange={(view) => {
					setLogsJumpToId(null);
					setActiveView(view);
					if (view === "settings" && guideState === "pending") {
						setSettingsGuideOpen(true);
					}
				}}
			/>
			{renderView()}

			<SettingsGuideOverlay
				opened={settingsGuideOpen}
				onSkip={() => {
					setSettingsGuideOpen(false);
					setGuideState.mutate("skipped");
				}}
				onFinished={() => {
					setSettingsGuideOpen(false);
					setGuideState.mutate("completed");
				}}
				onGoHome={() => {
					setActiveView("home");
				}}
			/>
		</div>
	);
}
