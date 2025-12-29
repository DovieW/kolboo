import {
	Accordion,
	ActionIcon,
	Badge,
	Box,
	Button,
	Code,
	CopyButton,
	Group,
	Paper,
	Stack,
	Text,
	Title,
	Tooltip,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { listen } from "@tauri-apps/api/event";
import { useDisclosure } from "@mantine/hooks";
import {
  AlertCircle,
  AlertTriangle,
  Bug,
  CheckCircle,
  Clock,
  Copy,
  Info,
  Loader,
  Pause,
  Play,
  Trash2,
  XCircle,
  Zap,
} from "lucide-react";
import { useEffect, useState } from "react";
import { useClearRequestLogs, useRequestLogs } from "../lib/queries";
import { useRecordingPlayer } from "../lib/useRecordingPlayer";
import { LogJsonModal } from "./LogJsonModal";
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
                variant="subtle"
                color="gray"
                size="sm"
                loading={player.isLoading(log.id)}
                disabled={log.status === "in_progress"}
                onClick={(e) => {
                  // Prevent accordion toggle when clicking play/pause.
                  e.preventDefault();
                  e.stopPropagation();
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
                    {log.final_text &&
                      (log.final_text !== log.raw_transcript ||
                        llmAttempted) && (
                        <Box>
                          <Text size="xs" fw={600} c="dimmed">
                            Final Output:
                          </Text>
                          <Text size="sm" style={{ whiteSpace: "pre-wrap" }}>
                            {log.final_text}
                          </Text>
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

export function LogsView() {
  const { data: logs } = useRequestLogs(100);
  const clearLogsMutation = useClearRequestLogs();
  const [systemEvents, setSystemEvents] = useState<SystemEvent[]>([]);
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

      {/* System Events Panel */}
      {systemEvents.length > 0 && (
        <Paper
          withBorder
          p="sm"
          style={{ background: "var(--mantine-color-dark-8)" }}
        >
          <Group justify="space-between" mb="xs">
            <Group gap="xs">
              <Zap
                size={16}
                style={{ color: "var(--mantine-color-yellow-5)" }}
              />
              <Text size="sm" fw={600}>
                System Events (Live)
              </Text>
            </Group>
            <Group gap="xs">
              <CopyButton value={JSON.stringify(systemEvents, null, 2)}>
                {({ copied, copy }) => (
                  <Button
                    variant="subtle"
                    color={copied ? "teal" : "gray"}
                    size="xs"
                    leftSection={<Copy size={12} />}
                    onClick={copy}
                  >
                    {copied ? "Copied!" : "Copy All"}
                  </Button>
                )}
              </CopyButton>
              <Button
                variant="subtle"
                color="gray"
                size="xs"
                onClick={() => setSystemEvents([])}
              >
                Clear
              </Button>
            </Group>
          </Group>
          <Stack gap={4} style={{ maxHeight: 200, overflowY: "auto" }}>
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
        </Paper>
      )}

      {logs && logs.length > 0 ? (
        <Accordion variant="contained" radius="md" chevronPosition="left">
          {logs.map((log) => (
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
            No request logs yet. Start a voice transcription to see logs here.
          </Text>
        </Paper>
      )}
    </Stack>
  );
}
