import {
  Accordion,
  ActionIcon,
  Badge,
  Box,
  Button,
  Code,
  CopyButton,
  Collapse,
  Divider,
  Group,
  Indicator,
  NumberInput,
  Paper,
  Popover,
  Stack,
  Text,
  TextInput,
  Title,
  Tooltip,
  UnstyledButton,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { listen } from "@tauri-apps/api/event";
import { useDisclosure } from "@mantine/hooks";
import {
  AlertCircle,
  AlertTriangle,
  Bug,
  CheckCircle,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  ChevronsLeft,
  ChevronsRight,
  Clock,
  Copy,
  Filter,
  Info,
  Loader,
  Pause,
  Play,
  Search,
  Trash2,
  X,
  XCircle,
  Zap,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useClearRequestLogs, useRequestLogs } from "../lib/queries";
import { useRecordingPlayer } from "../lib/useRecordingPlayer";
import { diffTextInline } from "../lib/textDiff";
import { LogJsonModal } from "./LogJsonModal";
import { InlineTextDiff } from "./InlineTextDiff";
import type {
  LogEntry,
  LogLevel,
  RequestLog,
  RequestStatus,
} from "../lib/tauri";

// System event from Rust backend
interface SystemEvent {
  timestamp: string;
  event_type: string;
  message: string;
  details: string | null;
}

function formatTimestamp(timestamp: string): string {
  const date = new Date(timestamp);
  return date.toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(2)}s`;
}

function formatUsdFromMicros(micros: number): string {
  const dollars = micros / 1_000_000;

  let fixed: string;
  if (dollars >= 10) fixed = dollars.toFixed(2);
  else if (dollars >= 1) fixed = dollars.toFixed(3);
  else if (dollars >= 0.1) fixed = dollars.toFixed(4);
  else fixed = dollars.toFixed(6);

  // Trim trailing zeros for compact chips.
  fixed = fixed.replace(/\.0+$/, "").replace(/(\.\d*[1-9])0+$/, "$1");
  return `$${fixed}`;
}

function formatCallPriceLabel(params: {
  isFreeTier: boolean;
  estimatedCostUsdMicros: number | null | undefined;
}): string {
  if (params.isFreeTier) return "$0 (free)";
  if (typeof params.estimatedCostUsdMicros === "number") {
    return formatUsdFromMicros(params.estimatedCostUsdMicros);
  }
  return "—";
}

function getStatusBadge(status: RequestStatus) {
  switch (status) {
    case "success":
      return (
        <Badge color="green" leftSection={<CheckCircle size={12} />}>
          Success
        </Badge>
      );
    case "error":
      return (
        <Badge color="red" leftSection={<XCircle size={12} />}>
          Error
        </Badge>
      );
    case "cancelled":
      return (
        <Badge color="yellow" leftSection={<AlertCircle size={12} />}>
          Cancelled
        </Badge>
      );
    case "in_progress":
      return (
        <Badge
          color="orange"
          leftSection={<Loader size={12} className="animate-spin" />}
        >
          In Progress
        </Badge>
      );
    default:
      return <Badge color="gray">{status}</Badge>;
  }
}

function getLogLevelIcon(level: LogLevel) {
  switch (level) {
    case "debug":
      return <Bug size={14} style={{ color: "var(--mantine-color-dimmed)" }} />;
    case "info":
      return (
        <Info size={14} style={{ color: "var(--mantine-color-blue-5)" }} />
      );
    case "warn":
      return (
        <AlertTriangle
          size={14}
          style={{ color: "var(--mantine-color-yellow-5)" }}
        />
      );
    case "error":
      return (
        <AlertCircle
          size={14}
          style={{ color: "var(--mantine-color-red-5)" }}
        />
      );
    default:
      return null;
  }
}

function getLogLevelColor(level: LogLevel): string {
  switch (level) {
    case "debug":
      return "dimmed";
    case "info":
      return "blue";
    case "warn":
      return "yellow";
    case "error":
      return "red";
    default:
      return "gray";
  }
}

function LogEntryItem({ entry }: { entry: LogEntry }) {
  const time = new Date(entry.timestamp).toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    fractionalSecondDigits: 3,
  });

  return (
    <Group gap="xs" align="flex-start" wrap="nowrap">
      <Text size="xs" c="dimmed" ff="monospace" style={{ minWidth: 85 }}>
        {time}
      </Text>
      {getLogLevelIcon(entry.level)}
      <Box style={{ flex: 1 }}>
        <Text size="sm" c={getLogLevelColor(entry.level)}>
          {entry.message}
        </Text>
        {entry.details && (
          <Code block mt={4} style={{ fontSize: "0.75rem" }}>
            {entry.details}
          </Code>
        )}
      </Box>
    </Group>
  );
}

function RequestLogItem({
  log,
  player,
}: {
  log: RequestLog;
  player: ReturnType<typeof useRecordingPlayer>;
}) {
  const [jsonOpened, jsonModal] = useDisclosure(false);

  // NOTE: `llm_provider`/`llm_model` can reflect configured defaults.
  // Use `llm_duration_ms` to indicate whether an LLM rewrite was actually attempted.
  const llmAttempted = log.llm_duration_ms !== null;
  const totalDurationMs = (() => {
    // Prefer backend-provided duration. This excludes recording time and matches
    // "request processing" time (stop -> STT/LLM -> done).
    if (typeof log.total_duration_ms === "number") return log.total_duration_ms;
    if (!log.ended_at) return null;
    const start = Date.parse(log.started_at);
    const end = Date.parse(log.ended_at);
    if (!Number.isFinite(start) || !Number.isFinite(end)) return null;
    if (end < start) return null;
    return end - start;
  })();
  const llmProviderLabel = log.llm_provider ?? "unknown";
  const sttMetaLabel = `${log.stt_provider}${
    log.stt_model ? ` / ${log.stt_model}` : ""
  }`;
  const llmMetaLabel = `${llmProviderLabel}${
    log.llm_model ? ` / ${log.llm_model}` : ""
  }`;

  // Always show a profile badge in request logs.
  // If the backend didn't populate profile fields (legacy logs), assume Default.
  const profileLabel = (() => {
    const name = (log.profile_name ?? "").trim();
    const id = (log.profile_id ?? "").trim();

    if (name) return name;
    if (!id || id === "default") return "Default";
    return id;
  })();

  const sttPriceLabel = formatCallPriceLabel({
    isFreeTier: log.stt_is_free_tier,
    estimatedCostUsdMicros: log.stt_estimated_cost_usd_micros,
  });
  const llmPriceLabel = formatCallPriceLabel({
    isFreeTier: log.llm_is_free_tier,
    estimatedCostUsdMicros: log.llm_estimated_cost_usd_micros,
  });

  const rawTranscriptTrimmed = (log.raw_transcript ?? "").trim();
  const finalOutputTrimmed = (log.final_text ?? "").trim();
  const hasAnyTranscriptText = !!(rawTranscriptTrimmed || finalOutputTrimmed);
  const playDisabled = log.status === "in_progress";
  const showRewriteDiff =
    llmAttempted &&
    typeof log.raw_transcript === "string" &&
    typeof log.final_text === "string" &&
    log.raw_transcript !== log.final_text;

  const differenceInfo = useMemo(() => {
    if (!showRewriteDiff) return null;

    const before = log.raw_transcript ?? "";
    const after = log.final_text ?? "";
    const chunks = diffTextInline(before, after);

    let equalChars = 0;
    let changedChars = 0;
    for (const c of chunks) {
      if (c.added || c.removed) changedChars += c.value.length;
      else equalChars += c.value.length;
    }

    const total = equalChars + changedChars;
    const changeRatio = total > 0 ? changedChars / total : 0;

    const tooLarge = changeRatio >= 0.62 || equalChars < 24;
    if (tooLarge) return null;

    // Count "change groups" (roughly: edit hunks). Treat a delete+insert
    // sequence with no unchanged text between as one change.
    let changeGroups = 0;
    let inGroup = false;
    for (const c of chunks) {
      const changed = Boolean(c.added) || Boolean(c.removed);
      if (!changed) {
        inGroup = false;
        continue;
      }
      if (!inGroup) {
        changeGroups += 1;
        inGroup = true;
      }
    }

    return { chunks, changeGroups };
  }, [showRewriteDiff, log.raw_transcript, log.final_text]);

  return (
    <Accordion.Item value={log.id} data-status={log.status}>
      <Accordion.Control>
        <Group justify="space-between" wrap="nowrap" pr="md">
          <Group gap="sm" wrap="nowrap">
            <Text size="sm" c="dimmed" ff="monospace">
              {formatTimestamp(log.started_at)}
            </Text>
          </Group>
          <Group gap="xs" wrap="nowrap">
            <Tooltip
              label={
                log.status === "in_progress"
                  ? "Recording is still in progress"
                  : player.isPlaying(log.id)
                  ? "Pause recording"
                  : "Play recording"
              }
            >
              <ActionIcon
                component="span"
                role="button"
                tabIndex={playDisabled ? -1 : 0}
                aria-disabled={playDisabled ? true : undefined}
                variant="subtle"
                color="gray"
                size="sm"
                loading={player.isLoading(log.id)}
                disabled={playDisabled}
                onClick={(e) => {
                  // Prevent accordion toggle when clicking play/pause.
                  e.preventDefault();
                  e.stopPropagation();
                  if (playDisabled) return;
                  player.toggle(log.id);
                }}
                onKeyDown={(e) => {
                  if (e.key !== "Enter" && e.key !== " ") return;
                  // Prevent accordion toggle when activating play/pause.
                  e.preventDefault();
                  e.stopPropagation();
                  if (playDisabled) return;
                  player.toggle(log.id);
                }}
                aria-label={player.isPlaying(log.id) ? "Pause" : "Play"}
              >
                {player.isPlaying(log.id) ? (
                  <Pause size={14} />
                ) : (
                  <Play size={14} />
                )}
              </ActionIcon>
            </Tooltip>
            {totalDurationMs !== null && (
              <Badge
                variant="light"
                size="sm"
                color="violet"
                leftSection={<Clock size={12} />}
              >
                Total: {formatDuration(totalDurationMs)}
              </Badge>
            )}
            {getStatusBadge(log.status)}
          </Group>
        </Group>
      </Accordion.Control>
      <Accordion.Panel>
        <Stack gap="md">
          {/* Transcript info */}
          {(log.raw_transcript || log.final_text) && (
            <Paper withBorder p="sm">
              <Stack gap="xs">
                {llmAttempted ? (
                  <>
                    {log.raw_transcript && (
                      <Box>
                        <Text size="xs" fw={600} c="dimmed">
                          Raw Transcript:
                        </Text>
                        <Text size="sm" style={{ whiteSpace: "pre-wrap" }}>
                          {log.raw_transcript || "(empty)"}
                        </Text>
                      </Box>
                    )}
                    {log.final_text && (
                      <Box>
                        <Text size="xs" fw={600} c="dimmed">
                          Rewrite Output:
                        </Text>
                        {typeof log.raw_transcript === "string" &&
                        typeof log.final_text === "string" &&
                        log.final_text === log.raw_transcript ? (
                          <Text size="sm" c="dimmed">
                            (no change)
                          </Text>
                        ) : (
                          <Text size="sm" style={{ whiteSpace: "pre-wrap" }}>
                            {log.final_text}
                          </Text>
                        )}
                      </Box>
                    )}

                    {differenceInfo && (
                      <Box>
                        <Text size="xs" fw={600} c="dimmed">
                          Difference ({differenceInfo.changeGroups} change
                          {differenceInfo.changeGroups === 1 ? "" : "s"}):
                        </Text>
                        <InlineTextDiff chunks={differenceInfo.chunks} />
                      </Box>
                    )}
                  </>
                ) : (
                  <Box>
                    <Text size="xs" fw={600} c="dimmed">
                      Transcript:
                    </Text>
                    <Text size="sm" style={{ whiteSpace: "pre-wrap" }}>
                      {log.final_text ?? log.raw_transcript ?? "(empty)"}
                    </Text>
                  </Box>
                )}
              </Stack>
            </Paper>
          )}

          {/* Error message */}
          {log.error_message && (
            <Paper
              withBorder
              p="sm"
              style={{ borderColor: "var(--mantine-color-red-5)" }}
            >
              <Group gap="xs" align="flex-start" justify="space-between">
                <Group gap="xs" align="flex-start" style={{ flex: 1 }}>
                  <AlertCircle
                    size={16}
                    style={{
                      color: "var(--mantine-color-red-5)",
                      flexShrink: 0,
                    }}
                  />
                  <Box style={{ flex: 1 }}>
                    <Text size="xs" fw={600} c="red">
                      Error:
                    </Text>
                    <Text size="sm" c="red" style={{ wordBreak: "break-word" }}>
                      {log.error_message}
                    </Text>
                  </Box>
                </Group>
                <CopyButton value={log.error_message}>
                  {({ copied, copy }) => (
                    <Tooltip label={copied ? "Copied!" : "Copy error"}>
                      <ActionIcon
                        variant="subtle"
                        color={copied ? "teal" : "gray"}
                        onClick={copy}
                        size="sm"
                      >
                        <Copy size={14} />
                      </ActionIcon>
                    </Tooltip>
                  )}
                </CopyButton>
              </Group>
            </Paper>
          )}

          {/* Timing and model info - always show model info even for failed requests */}
          <Group gap="xs" wrap="wrap">
            {log.stt_duration_ms ? (
              <Badge variant="light" size="sm" color="gray">
                STT {formatDuration(log.stt_duration_ms)} · {sttMetaLabel} ·{" "}
                {sttPriceLabel}
              </Badge>
            ) : (
              <Badge variant="light" size="sm" color="gray">
                STT · {sttMetaLabel} · {sttPriceLabel}
              </Badge>
            )}
            {log.llm_duration_ms ? (
              <Badge variant="light" size="sm" color="gray">
                LLM {formatDuration(log.llm_duration_ms)} · {llmMetaLabel} ·{" "}
                {llmPriceLabel}
              </Badge>
            ) : llmAttempted || log.llm_provider ? (
              <Badge variant="light" size="sm" color="gray">
                LLM · {llmMetaLabel} · {llmPriceLabel}
              </Badge>
            ) : null}

            <Badge variant="light" size="sm" color="gray">
              Profile · {profileLabel}
            </Badge>
          </Group>

          {/* Log entries */}
          {log.entries.length > 0 && (
            <Box>
              <Text size="xs" fw={600} c="dimmed" mb="xs">
                Log Entries ({log.entries.length}):
              </Text>
              <Paper
                withBorder
                p="sm"
                style={{ background: "var(--mantine-color-dark-8)" }}
              >
                <Stack gap={4}>
                  {log.entries.map((entry, index) => (
                    <LogEntryItem
                      key={`${entry.timestamp}-${index}`}
                      entry={entry}
                    />
                  ))}
                </Stack>
              </Paper>
            </Box>
          )}

          {/* Copy full log as JSON for debugging */}
          <Group justify="flex-end" gap={4}>
            {hasAnyTranscriptText && (
              <CopyButton value={rawTranscriptTrimmed}>
                {({ copied, copy }) => (
                  <Button
                    variant="subtle"
                    color={copied ? "teal" : "gray"}
                    size="xs"
                    leftSection={<Copy size={14} />}
                    onClick={copy}
                  >
                    {copied ? "Copied!" : "Copy Raw"}
                  </Button>
                )}
              </CopyButton>
            )}
            {hasAnyTranscriptText && (
              <CopyButton value={finalOutputTrimmed}>
                {({ copied, copy }) => (
                  <Button
                    variant="subtle"
                    color={copied ? "teal" : "gray"}
                    size="xs"
                    leftSection={<Copy size={14} />}
                    onClick={copy}
                  >
                    {copied ? "Copied!" : "Copy Rewrite"}
                  </Button>
                )}
              </CopyButton>
            )}
            <Button
              variant="subtle"
              color="gray"
              size="xs"
              onClick={(e) => {
                // Avoid accordion toggle.
                e.preventDefault();
                e.stopPropagation();
                jsonModal.open();
              }}
            >
              JSON
            </Button>
          </Group>

          <LogJsonModal
            opened={jsonOpened}
            onClose={jsonModal.close}
            log={log}
          />
        </Stack>
      </Accordion.Panel>
    </Accordion.Item>
  );
}

export function LogsView(
  props: {
    jumpToLogId?: string | null;
    onJumpHandled?: () => void;
  } = {}
) {
  const { jumpToLogId = null, onJumpHandled } = props;
  const { data: logs } = useRequestLogs(50);
  const clearLogsMutation = useClearRequestLogs();
  const [systemEvents, setSystemEvents] = useState<SystemEvent[]>([]);
  const [systemEventsAccordionValue, setSystemEventsAccordionValue] = useState<
    string | null
  >(null);
  const [filterText, setFilterText] = useState("");
  const [filtersOpened, setFiltersOpened] = useState(false);
  const [filtersExpandedSection, setFiltersExpandedSection] = useState<
    "status" | "duration" | null
  >(null);

  const [openedLogId, setOpenedLogId] = useState<string | null>(null);

  // Status filters (in-progress is always included; these apply to completed logs).
  const [showSuccess, setShowSuccess] = useState(true);
  const [showError, setShowError] = useState(true);
  const [showCancelled, setShowCancelled] = useState(true);

  // Duration filter (seconds). Empty means no bound.
  const [durationMinSecs, setDurationMinSecs] = useState<string | number>("");
  const [durationMaxSecs, setDurationMaxSecs] = useState<string | number>("");

  const LOGS_PAGE_SIZE = 25;
  const [page, setPage] = useState(1);
  const player = useRecordingPlayer({
    onError: (message) => {
      notifications.show({
        title: "Playback",
        message,
        color: "red",
      });
    },
  });

  // Listen for system events from Rust
  useEffect(() => {
    const unlisten = listen<SystemEvent>("system-event", (event) => {
      setSystemEvents((prev) => [event.payload, ...prev].slice(0, 50)); // Keep last 50
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Jump in from elsewhere (e.g., History): ensure the target log is visible and expanded.
  useEffect(() => {
    if (!jumpToLogId) return;

    setFilterText(jumpToLogId);
    setShowSuccess(true);
    setShowError(true);
    setShowCancelled(true);
    setDurationMinSecs("");
    setDurationMaxSecs("");
    setFiltersOpened(false);
    setFiltersExpandedSection(null);
    setPage(1);
    setOpenedLogId(jumpToLogId);

    onJumpHandled?.();
  }, [jumpToLogId, onJumpHandled]);

  const hasActiveFilters =
    !showSuccess ||
    !showError ||
    !showCancelled ||
    String(durationMinSecs).trim().length > 0 ||
    String(durationMaxSecs).trim().length > 0;

  const resetFilters = () => {
    setShowSuccess(true);
    setShowError(true);
    setShowCancelled(true);
    setDurationMinSecs("");
    setDurationMaxSecs("");
  };

  const filteredLogs = useMemo(() => {
    if (!logs) return [];
    const query = filterText.trim().toLowerCase();

    const minRaw =
      typeof durationMinSecs === "number"
        ? durationMinSecs
        : Number.parseFloat(durationMinSecs);
    const maxRaw =
      typeof durationMaxSecs === "number"
        ? durationMaxSecs
        : Number.parseFloat(durationMaxSecs);

    const minMs = Number.isFinite(minRaw) && minRaw >= 0 ? minRaw * 1000 : null;
    const maxMs = Number.isFinite(maxRaw) && maxRaw >= 0 ? maxRaw * 1000 : null;

    const getTotalDurationMs = (log: RequestLog): number | null => {
      if (typeof log.total_duration_ms === "number")
        return log.total_duration_ms;
      // In-progress has no ended_at (and should always be included anyway).
      if (!log.ended_at) return null;
      const start = Date.parse(log.started_at);
      const end = Date.parse(log.ended_at);
      if (!Number.isFinite(start) || !Number.isFinite(end)) return null;
      if (end < start) return null;
      return end - start;
    };

    return logs.filter((log) => {
      // Always include in-progress regardless of filters (per requirement).
      if (log.status === "in_progress") return true;

      // Text search
      if (query) {
        const haystack = [
          log.id,
          log.error_message ?? "",
          log.raw_transcript ?? "",
          log.final_text ?? "",
        ]
          .join("\n")
          .toLowerCase();
        if (!haystack.includes(query)) return false;
      }

      // Status toggles
      if (!showSuccess && log.status === "success") return false;
      if (!showError && log.status === "error") return false;
      if (!showCancelled && log.status === "cancelled") return false;

      // Duration filter
      if (minMs !== null || maxMs !== null) {
        const totalMs = getTotalDurationMs(log);
        // If duration is missing for a completed log, keep it (best-effort).
        if (typeof totalMs === "number") {
          if (minMs !== null && totalMs < minMs) return false;
          if (maxMs !== null && totalMs > maxMs) return false;
        }
      }

      return true;
    });
  }, [
    logs,
    filterText,
    showSuccess,
    showError,
    showCancelled,
    durationMinSecs,
    durationMaxSecs,
  ]);

  const totalPages = Math.max(
    1,
    Math.ceil(filteredLogs.length / LOGS_PAGE_SIZE)
  );
  const canGoPrev = page > 1;
  const canGoNext = page < totalPages;

  useEffect(() => {
    setPage((current) => Math.min(Math.max(1, current), totalPages));
  }, [totalPages]);

  useEffect(() => {
    setPage(1);
  }, [
    filterText,
    showSuccess,
    showError,
    showCancelled,
    durationMinSecs,
    durationMaxSecs,
  ]);

  const pageLogs = useMemo(() => {
    const start = (page - 1) * LOGS_PAGE_SIZE;
    return filteredLogs.slice(start, start + LOGS_PAGE_SIZE);
  }, [filteredLogs, page]);

  return (
    <Stack gap="md" style={{ width: "100%" }}>
      <Group justify="space-between" align="center">
        <Title order={3}>Request Logs</Title>
        <Group gap="xs">
          <Button
            variant="subtle"
            color="red"
            size="xs"
            leftSection={<Trash2 size={14} />}
            onClick={() => clearLogsMutation.mutate()}
            loading={clearLogsMutation.isPending}
            disabled={!logs || logs.length === 0}
          >
            Clear All
          </Button>
        </Group>
      </Group>

      <Text size="sm" c="dimmed">
        View detailed logs of voice transcription requests. Logs are stored in
        memory and cleared on app restart.
      </Text>

      {/* Filters + Pagination */}
      <Group gap={12} align="center" wrap="wrap">
        <TextInput
          value={filterText}
          onChange={(e) => setFilterText(e.currentTarget.value)}
          placeholder="Filter request logs…"
          leftSection={<Search size={14} />}
          rightSection={
            filterText.trim().length > 0 ? (
              <ActionIcon
                variant="subtle"
                size="sm"
                color="gray"
                onClick={() => setFilterText("")}
                title="Clear filter"
              >
                <X size={14} />
              </ActionIcon>
            ) : null
          }
          styles={{
            input: {
              backgroundColor: "transparent",
              borderColor: "var(--border-default)",
              color: "var(--text-primary)",
            },
          }}
          size="xs"
          style={{ width: 260 }}
        />

        <Popover
          opened={filtersOpened}
          onChange={setFiltersOpened}
          position="bottom-start"
          shadow="lg"
          radius="md"
        >
          <Popover.Target>
            <Indicator
              size={8}
              color="orange"
              offset={2}
              disabled={!hasActiveFilters}
              processing={hasActiveFilters}
            >
              <ActionIcon
                variant={hasActiveFilters ? "light" : "subtle"}
                size="sm"
                color={hasActiveFilters ? "orange" : "gray"}
                onClick={() => setFiltersOpened((v) => !v)}
                title="Filter options"
                aria-label="Filter options"
              >
                <Filter size={16} />
              </ActionIcon>
            </Indicator>
          </Popover.Target>
          <Popover.Dropdown
            p={0}
            style={{
              backgroundColor: "var(--bg-elevated)",
              border: "1px solid var(--border-default)",
              width: 300,
              overflow: "hidden",
            }}
          >
            <Group justify="space-between" p="xs" pb={8}>
              <Text size="sm" fw={600}>
                Filters
              </Text>
              {hasActiveFilters && (
                <Button
                  variant="subtle"
                  size="compact-xs"
                  color="gray"
                  onClick={resetFilters}
                  styles={{ root: { height: 20, padding: "0 6px" } }}
                >
                  Reset
                </Button>
              )}
            </Group>

            <Divider color="var(--border-default)" />

            {/* Status Section */}
            <Box>
              <UnstyledButton
                onClick={() =>
                  setFiltersExpandedSection((current) =>
                    current === "status" ? null : "status"
                  )
                }
                w="100%"
                py={8}
                px="xs"
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                }}
              >
                <Text size="xs" fw={500}>
                  Status
                </Text>
                <ChevronDown
                  size={14}
                  style={{
                    transform:
                      filtersExpandedSection === "status"
                        ? "rotate(180deg)"
                        : "rotate(0)",
                    transition: "transform 150ms ease",
                    color: "var(--text-secondary)",
                  }}
                />
              </UnstyledButton>
              <Collapse in={filtersExpandedSection === "status"}>
                <Stack gap={0} p="xs" pt={0}>
                  <Group justify="space-between" py={4}>
                    <Text size="xs">Show success</Text>
                    <Button
                      variant={showSuccess ? "light" : "subtle"}
                      size="compact-xs"
                      color={showSuccess ? "green" : "gray"}
                      onClick={() => setShowSuccess((v) => !v)}
                    >
                      {showSuccess ? "On" : "Off"}
                    </Button>
                  </Group>
                  <Group justify="space-between" py={4}>
                    <Text size="xs">Show errors</Text>
                    <Button
                      variant={showError ? "light" : "subtle"}
                      size="compact-xs"
                      color={showError ? "red" : "gray"}
                      onClick={() => setShowError((v) => !v)}
                    >
                      {showError ? "On" : "Off"}
                    </Button>
                  </Group>
                  <Group justify="space-between" py={4}>
                    <Text size="xs">Show cancelled</Text>
                    <Button
                      variant={showCancelled ? "light" : "subtle"}
                      size="compact-xs"
                      color={showCancelled ? "yellow" : "gray"}
                      onClick={() => setShowCancelled((v) => !v)}
                    >
                      {showCancelled ? "On" : "Off"}
                    </Button>
                  </Group>
                  <Text size="xs" c="dimmed" mt={6}>
                    In-progress requests are always shown.
                  </Text>
                </Stack>
              </Collapse>
            </Box>

            <Divider color="var(--border-default)" />

            {/* Duration Section */}
            <Box>
              <UnstyledButton
                onClick={() =>
                  setFiltersExpandedSection((current) =>
                    current === "duration" ? null : "duration"
                  )
                }
                w="100%"
                py={8}
                px="xs"
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                }}
              >
                <Text size="xs" fw={500}>
                  Duration
                </Text>
                <ChevronDown
                  size={14}
                  style={{
                    transform:
                      filtersExpandedSection === "duration"
                        ? "rotate(180deg)"
                        : "rotate(0)",
                    transition: "transform 150ms ease",
                    color: "var(--text-secondary)",
                  }}
                />
              </UnstyledButton>
              <Collapse in={filtersExpandedSection === "duration"}>
                <Stack gap="xs" p="xs" pt={0}>
                  <Group grow>
                    <NumberInput
                      label="Min (sec)"
                      value={durationMinSecs}
                      onChange={setDurationMinSecs}
                      min={0}
                      step={0.5}
                      hideControls
                      size="xs"
                      styles={{
                        input: {
                          backgroundColor: "transparent",
                          borderColor: "var(--border-default)",
                          color: "var(--text-primary)",
                        },
                      }}
                    />
                    <NumberInput
                      label="Max (sec)"
                      value={durationMaxSecs}
                      onChange={setDurationMaxSecs}
                      min={0}
                      step={0.5}
                      hideControls
                      size="xs"
                      styles={{
                        input: {
                          backgroundColor: "transparent",
                          borderColor: "var(--border-default)",
                          color: "var(--text-primary)",
                        },
                      }}
                    />
                  </Group>
                  <Text size="xs" c="dimmed">
                    Applies to completed requests. In-progress requests are
                    always included.
                  </Text>
                </Stack>
              </Collapse>
            </Box>
          </Popover.Dropdown>
        </Popover>

        <Text c="dimmed" size="xs" style={{ whiteSpace: "nowrap" }}>
          {filteredLogs.length} result{filteredLogs.length === 1 ? "" : "s"}
        </Text>

        <Group style={{ marginLeft: "auto" }} gap={6}>
          <ActionIcon
            variant="subtle"
            size="sm"
            color="gray"
            onClick={() => setPage(1)}
            disabled={!canGoPrev}
            title="First page"
          >
            <ChevronsLeft size={16} />
          </ActionIcon>
          <ActionIcon
            variant="subtle"
            size="sm"
            color="gray"
            onClick={() => setPage((p) => Math.max(1, p - 1))}
            disabled={!canGoPrev}
            title="Previous page"
          >
            <ChevronLeft size={16} />
          </ActionIcon>
          <ActionIcon
            variant="subtle"
            size="sm"
            color="gray"
            onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
            disabled={!canGoNext}
            title="Next page"
          >
            <ChevronRight size={16} />
          </ActionIcon>
          <ActionIcon
            variant="subtle"
            size="sm"
            color="gray"
            onClick={() => setPage(totalPages)}
            disabled={!canGoNext}
            title="Last page"
          >
            <ChevronsRight size={16} />
          </ActionIcon>
        </Group>
      </Group>

      {/* System Events Panel */}
      {systemEvents.length > 0 && (
        <Accordion
          variant="contained"
          radius="md"
          chevronPosition="right"
          value={systemEventsAccordionValue}
          onChange={setSystemEventsAccordionValue}
        >
          <Accordion.Item value="system-events">
            <Accordion.Control>
              <Group justify="space-between" wrap="nowrap" pr="xs">
                <Group gap="xs" wrap="nowrap">
                  <Zap
                    size={16}
                    style={{ color: "var(--mantine-color-yellow-5)" }}
                  />
                  <Text size="sm" fw={600}>
                    System Events (Live)
                  </Text>
                  <Badge size="xs" variant="light" color="gray">
                    {systemEvents.length}
                  </Badge>
                </Group>

                <Group gap="xs" wrap="nowrap">
                  <CopyButton value={JSON.stringify(systemEvents, null, 2)}>
                    {({ copied, copy }) => (
                      <Button
                        variant="subtle"
                        color={copied ? "teal" : "gray"}
                        size="xs"
                        leftSection={<Copy size={12} />}
                        onClick={(e) => {
                          // Prevent toggling the accordion when clicking actions.
                          e.preventDefault();
                          e.stopPropagation();
                          copy();
                        }}
                      >
                        {copied ? "Copied!" : "Copy All"}
                      </Button>
                    )}
                  </CopyButton>

                  <Button
                    variant="subtle"
                    color="gray"
                    size="xs"
                    onClick={(e) => {
                      // Prevent toggling the accordion when clicking actions.
                      e.preventDefault();
                      e.stopPropagation();
                      setSystemEvents([]);
                      setSystemEventsAccordionValue(null);
                    }}
                  >
                    Clear
                  </Button>
                </Group>
              </Group>
            </Accordion.Control>
            <Accordion.Panel>
              <Stack gap={4} style={{ maxHeight: 120, overflowY: "auto" }}>
                {systemEvents.map((event, idx) => (
                  <Group
                    key={`${event.timestamp}-${idx}`}
                    gap="xs"
                    wrap="nowrap"
                    align="flex-start"
                  >
                    <Text
                      size="xs"
                      c="dimmed"
                      ff="monospace"
                      style={{
                        whiteSpace: "nowrap",
                        minWidth: 92,
                        textAlign: "right",
                        fontVariantNumeric: "tabular-nums",
                      }}
                    >
                      {new Date(event.timestamp).toLocaleTimeString(undefined, {
                        hour: "2-digit",
                        minute: "2-digit",
                        second: "2-digit",
                      })}
                    </Text>
                    <Badge
                      size="xs"
                      color={
                        event.event_type === "error"
                          ? "red"
                          : event.event_type === "shortcut"
                          ? "blue"
                          : "gray"
                      }
                    >
                      {event.event_type}
                    </Badge>
                    <Text size="xs" style={{ flex: 1 }}>
                      {event.message}
                      {event.details && (
                        <Text span c="dimmed" size="xs">
                          {" "}
                          - {event.details}
                        </Text>
                      )}
                    </Text>
                  </Group>
                ))}
              </Stack>
            </Accordion.Panel>
          </Accordion.Item>
        </Accordion>
      )}

      {pageLogs && pageLogs.length > 0 ? (
        <Accordion
          variant="contained"
          radius="md"
          chevronPosition="left"
          value={openedLogId}
          onChange={setOpenedLogId}
        >
          {pageLogs.map((log) => (
            <RequestLogItem key={log.id} log={log} player={player} />
          ))}
        </Accordion>
      ) : (
        <Paper withBorder p="xl" ta="center">
          <Info
            size={32}
            style={{ color: "var(--mantine-color-dimmed)", margin: "0 auto" }}
          />
          <Text size="sm" c="dimmed" mt="sm">
            {logs && logs.length > 0
              ? "No matches. Try a different filter."
              : "No request logs yet. Start a voice transcription to see logs here."}
          </Text>
        </Paper>
      )}
    </Stack>
  );
}
