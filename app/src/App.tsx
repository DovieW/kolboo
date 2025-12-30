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
  Stack,
  Tabs,
  Text,
  Title,
  Tooltip,
} from "@mantine/core";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  BarChart2,
  CircleHelp,
  Cog,
  Filter,
  FileText,
  Home,
  Plus,
  Settings,
} from "lucide-react";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { HistoryFeed } from "./components/HistoryFeed";
import { Logo } from "./components/Logo";
import { LogsView } from "./components/LogsView";
import {
  ApiKeysSettings,
  AudioSettings,
  DataSettings,
  HotkeySettings,
  PromptSettings,
  ProfileConfigModal,
  UiSettings,
} from "./components/settings";
import { SettingsGuideOverlay } from "./components/settings/SettingsGuideOverlay";
import { API_KEY_STORE_KEYS } from "./components/settings/ApiKeysSettings";
import {
  DEFAULT_HOLD_HOTKEY,
  DEFAULT_PASTE_LAST_HOTKEY,
  DEFAULT_TOGGLE_HOTKEY,
} from "./lib/hotkeyDefaults";
import { applyAccentColor } from "./lib/accentColor";
import {
  useSetSettingsGuideState,
  useSettings,
  useSettingsGuideState,
} from "./lib/queries";
import { listAllLlmModelKeys, listAllSttModelKeys } from "./lib/modelOptions";
import { type CostTimeframe, type HotkeyConfig, tauriAPI } from "./lib/tauri";
import { CostTab, type StatsKindFilter } from "./components/usageStats/CostTab";
import "./styles.css";

type View = "home" | "settings" | "logs" | "usage-stats";

function readBootGuideState(): "pending" | "skipped" | "completed" | null {
  try {
    if (typeof window === "undefined" || !window.localStorage) return null;
    const raw = window.localStorage.getItem("tv_settings_guide_state");
    if (raw === "pending" || raw === "skipped" || raw === "completed")
      return raw;
    return null;
  } catch {
    return null;
  }
}

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

function Sidebar({
  activeView,
  onViewChange,
}: {
  activeView: View;
  onViewChange: (view: View) => void;
}) {
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
        <a
          className="sidebar-footer-link"
          href="https://github.com/DovieW/kolboo"
          target="_blank"
          rel="noreferrer"
        >
          v0.1.0
        </a>
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

function InstructionsCard() {
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
      <header className="animate-in" style={{ marginBottom: 32 }}>
        <Title order={1} mb={4}>
          Welcome to Kolboo
        </Title>
        <Text c="dimmed" size="sm">
          ~-~-~-~-~-~
        </Text>
      </header>

      <InstructionsCard />

      <HistoryFeed onJumpToLog={onJumpToLog} />
    </div>
  );
}

function UsageStatsView() {
  const [activeStatsTab, setActiveStatsTab] = useState<string>("cost");
  const [timeframe, setTimeframe] = useState<CostTimeframe>("30d");
  const [filtersOpened, setFiltersOpened] = useState(false);

  const [statsKind, setStatsKind] = useState<StatsKindFilter>("all");
  const [selectedSttModelKeys, setSelectedSttModelKeys] = useState<string[]>(
    []
  );
  const [selectedLlmModelKeys, setSelectedLlmModelKeys] = useState<string[]>(
    []
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
      <header className="animate-in" style={{ marginBottom: 20 }}>
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
                              variant="subtle"
                              size="compact-xs"
                              color="gray"
                              onClick={(e) => {
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
                              variant="subtle"
                              size="compact-xs"
                              color="gray"
                              onClick={(e) => {
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
  );
}

function SettingsView() {
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
          API_KEY_STORE_KEYS.map((key) => tauriAPI.hasApiKey(key))
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
    { value: "default", label: "Default" },
    ...profiles.map((p) => ({
      value: p.id,
      label: p.name.trim() ? p.name.trim() : p.id,
    })),
  ];

  return (
    <div className="main-content">
      <header
        className="animate-in"
        style={{
          marginBottom: 20,
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
          <Tabs.Tab value="ai">AI</Tabs.Tab>
          <Tabs.Tab value="ui">UI</Tabs.Tab>
          <Tabs.Tab value="audio">Audio</Tabs.Tab>
          <Tabs.Tab value="hotkeys">Hotkeys</Tabs.Tab>
          <Tabs.Tab value="api-keys">API Keys</Tabs.Tab>
          <Tabs.Tab value="data">Data</Tabs.Tab>
        </Tabs.List>

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
      </Tabs>
    </div>
  );
}

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

  const { data: hasAnyApiKey } = useQuery({
    queryKey: ["hasAnyApiKey"],
    queryFn: async () => {
      try {
        const results = await Promise.all(
          API_KEY_STORE_KEYS.map((key) => tauriAPI.hasApiKey(key))
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
    { value: "default", label: "Default" },
    ...profiles.map((p) => ({
      value: p.id,
      label: p.name.trim() ? p.name.trim() : p.id,
    })),
  ];

  return (
    <div className="main-content">
      <header
        className="animate-in"
        style={{
          marginBottom: 20,
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
          <Tabs.Tab value="ai">AI</Tabs.Tab>
          <Tabs.Tab value="ui">UI</Tabs.Tab>
          <Tabs.Tab value="audio">Audio</Tabs.Tab>
          <Tabs.Tab value="hotkeys">Hotkeys</Tabs.Tab>
          <Tabs.Tab value="api-keys">API Keys</Tabs.Tab>
          <Tabs.Tab value="data">Data</Tabs.Tab>
        </Tabs.List>

        <Tabs.Panel value="ai" pt="md">
          <div className="settings-card">
            <PromptSettings editingProfileId={editingProfileId} />
          </div>
        </Tabs.Panel>

        <Tabs.Panel value="ui" pt="md">
          <div className="settings-card">
            <UiSettings
              editingProfileId={editingProfileId}
              onRunSetupGuide={onRunSetupGuide}
            />
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
      </Tabs>
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
  const queryClient = useQueryClient();

  const bootGuideState = readBootGuideState();
  const bootGuideKnown = bootGuideState !== null;
  const bootShouldAutoOpenGuide = bootGuideState === "pending";

  const [activeView, setActiveView] = useState<View>(() =>
    bootShouldAutoOpenGuide ? "settings" : "home"
  );
  const [logsJumpToId, setLogsJumpToId] = useState<string | null>(null);
  const [settingsGuideOpen, setSettingsGuideOpen] = useState<boolean>(
    () => bootShouldAutoOpenGuide
  );

  const { data: guideState } = useSettingsGuideState();
  const setGuideState = useSetSettingsGuideState();
  const { data: settings } = useSettings();

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

  // If we don't have a boot hint yet (first ever run, or storage was cleared),
  // avoid rendering the Home view for a moment before the guide state arrives.
  if (!bootGuideKnown && guideState === undefined) {
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

  useEffect(() => {
    if (guideState === "pending") {
      setActiveView("settings");
      setSettingsGuideOpen(true);
    }
  }, [guideState]);

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
