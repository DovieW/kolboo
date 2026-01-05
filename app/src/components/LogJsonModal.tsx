import { Box, Modal, Stack, Tabs, Text } from "@mantine/core";
import { CodeHighlight } from "@mantine/code-highlight";
import { useEffect, useMemo, useState } from "react";
import type { RequestLog } from "../lib/tauri";

type TabKey =
  | "full"
  | "stt-request"
  | "stt-response"
  | "llm-request"
  | "llm-response"
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
        copyLabel="Copy JSON"
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

export function LogJsonModal({
  opened,
  onClose,
  log,
}: {
  opened: boolean;
  onClose: () => void;
  log: RequestLog;
}) {
  const hasSttPayload =
    log.stt_request_json !== undefined || log.stt_response_json !== undefined;
  const hasLlmPayload =
    log.llm_request_json !== undefined || log.llm_response_json !== undefined;
  const hasRouterPayload =
    log.router_request_json !== undefined ||
    log.router_response_json !== undefined;

  const [tab, setTab] = useState<TabKey>("full");

  // Reset selection when opening / switching log rows.
  useEffect(() => {
    if (!opened) return;
    setTab("full");
  }, [opened, log.id, hasLlmPayload, hasRouterPayload]);

  return (
    <Modal
      opened={opened}
      onClose={onClose}
      title="JSON"
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
            <Tabs.Tab value="full">Full</Tabs.Tab>

            {hasSttPayload && (
              <>
                <Tabs.Tab value="stt-request">STT Request</Tabs.Tab>
                <Tabs.Tab value="stt-response">STT Response</Tabs.Tab>
              </>
            )}

            {hasLlmPayload && (
              <>
                <Tabs.Tab value="llm-request">LLM Request</Tabs.Tab>
                <Tabs.Tab value="llm-response">LLM Response</Tabs.Tab>
              </>
            )}

            {hasRouterPayload && (
              <>
                <Tabs.Tab value="router-request">Router Request</Tabs.Tab>
                <Tabs.Tab value="router-response">Router Response</Tabs.Tab>
              </>
            )}
          </Tabs.List>

          <Tabs.Panel
            value="full"
            pt="sm"
            style={{ flex: 1, minHeight: 0, overflow: "hidden" }}
          >
            <JsonPanel value={log} />
          </Tabs.Panel>

          {hasSttPayload && (
            <>
              <Tabs.Panel
                value="stt-request"
                pt="sm"
                style={{ flex: 1, minHeight: 0, overflow: "hidden" }}
              >
                <JsonPanel value={log.stt_request_json} />
              </Tabs.Panel>
              <Tabs.Panel
                value="stt-response"
                pt="sm"
                style={{ flex: 1, minHeight: 0, overflow: "hidden" }}
              >
                <JsonPanel value={log.stt_response_json} />
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
                <JsonPanel value={log.llm_request_json} />
              </Tabs.Panel>
              <Tabs.Panel
                value="llm-response"
                pt="sm"
                style={{ flex: 1, minHeight: 0, overflow: "hidden" }}
              >
                <JsonPanel value={log.llm_response_json} />
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
                <JsonPanel value={log.router_request_json} />
              </Tabs.Panel>
              <Tabs.Panel
                value="router-response"
                pt="sm"
                style={{ flex: 1, minHeight: 0, overflow: "hidden" }}
              >
                <JsonPanel value={log.router_response_json} />
              </Tabs.Panel>
            </>
          )}
        </Tabs>

        {!hasSttPayload && !hasLlmPayload && !hasRouterPayload && (
          <Text size="xs" c="dimmed">
            No STT/LLM/Router payloads captured for this request.
          </Text>
        )}
      </Stack>
    </Modal>
  );
}
