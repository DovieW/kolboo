import { Card, Group, Skeleton, Text, Title, Tooltip } from "@mantine/core";
import { useMemo } from "react";
import { useCostByProvider, useCostSummary } from "../../lib/queries";
import type { CostTimeframe } from "../../lib/tauri";

function formatUsdFromMicros(micros: number): {
  display: string;
  exact: string;
} {
  const safeMicros =
    typeof micros === "number" && Number.isFinite(micros) ? micros : 0;
  const dollars = safeMicros / 1_000_000;

  const exact = new Intl.NumberFormat(undefined, {
    style: "currency",
    currency: "USD",
    maximumFractionDigits: 6,
  }).format(dollars);

  // For tiny spends (e.g. a couple seconds of Whisper), rounding to cents looks like "no change".
  const display = new Intl.NumberFormat(undefined, {
    style: "currency",
    currency: "USD",
    minimumFractionDigits: dollars > 0 && dollars < 0.01 ? 4 : 2,
    maximumFractionDigits: dollars > 0 && dollars < 0.01 ? 6 : 2,
  }).format(dollars);

  return { display, exact };
}

export type StatsKind = "stt" | "llm";

export type StatsKindFilter = "all" | StatsKind;

export function CostTab(props: {
  timeframe: CostTimeframe;
  kind: StatsKindFilter;
  sttModelKeys: string[];
  llmModelKeys: string[];
  excludeFreeTier: boolean;
}) {
  const { timeframe, kind, sttModelKeys, llmModelKeys, excludeFreeTier } =
    props;
  const kindParam = kind === "all" ? undefined : kind;
  const summary = useCostSummary(timeframe, {
    kind: kindParam,
    sttModelKeys,
    llmModelKeys,
    excludeFreeTier,
  });
  const byProvider = useCostByProvider(timeframe, {
    kind: kindParam,
    sttModelKeys,
    llmModelKeys,
    excludeFreeTier,
  });

  const totalLabel = useMemo(() => {
    const micros = summary.data?.total_usd_micros ?? 0;
    return formatUsdFromMicros(micros);
  }, [summary.data?.total_usd_micros]);

  const kindLabel =
    kind === "all"
      ? "speech-to-text + LLM"
      : kind === "stt"
      ? "speech-to-text"
      : "LLM";

  const modelsLabelParts: string[] = [];
  if (kind === "stt") {
    if (sttModelKeys.length > 0)
      modelsLabelParts.push(`Filtered to ${sttModelKeys.length} STT model(s)`);
  } else if (kind === "llm") {
    if (llmModelKeys.length > 0)
      modelsLabelParts.push(`Filtered to ${llmModelKeys.length} LLM model(s)`);
  } else {
    if (sttModelKeys.length > 0)
      modelsLabelParts.push(`Filtered to ${sttModelKeys.length} STT model(s)`);
    if (llmModelKeys.length > 0)
      modelsLabelParts.push(`Filtered to ${llmModelKeys.length} LLM model(s)`);
  }
  if (excludeFreeTier) modelsLabelParts.push("Excluding free tier");
  const modelsLabel =
    modelsLabelParts.length > 0 ? modelsLabelParts.join(" • ") : null;

  return (
    <div className="animate-in" style={{ display: "grid", gap: 12 }}>
      <Group justify="space-between" align="center" wrap="wrap">
        <Title order={3}>Total spend</Title>
      </Group>

      <Card
        withBorder
        radius="md"
        style={{
          backgroundColor: "var(--bg-elevated)",
          borderColor: "var(--border-default)",
        }}
      >
        <Text c="dimmed" size="sm">
          Across {kindLabel}, all providers and models
          {modelsLabel ? ` • ${modelsLabel}` : ""}
        </Text>

        {summary.isLoading || summary.isFetching ? (
          <Skeleton height={34} width={180} mt={8} />
        ) : (
          <Tooltip label={`Exact: ${totalLabel.exact}`} withArrow>
            <Title order={2} mt={6} style={{ letterSpacing: -0.5 }}>
              {totalLabel.display}
            </Title>
          </Tooltip>
        )}

        {summary.isError ? (
          <Text size="sm" c="red" mt={6}>
            Failed to load cost stats.
          </Text>
        ) : null}
      </Card>

      <div style={{ display: "grid", gap: 8 }}>
        <Text size="sm" c="dimmed">
          Totals by provider
        </Text>

        {byProvider.isLoading || byProvider.isFetching ? (
          <div style={{ display: "grid", gap: 6 }}>
            <Skeleton height={18} width={240} />
            <Skeleton height={18} width={220} />
            <Skeleton height={18} width={200} />
          </div>
        ) : byProvider.isError ? (
          <Text size="sm" c="red">
            Failed to load provider totals.
          </Text>
        ) : byProvider.data?.providers?.length ? (
          <div style={{ display: "grid", gap: 6 }}>
            {byProvider.data.providers.map((p) => {
              const label = formatUsdFromMicros(p.total_usd_micros);
              return (
                <Group
                  key={p.provider}
                  justify="space-between"
                  gap="md"
                  wrap="nowrap"
                >
                  <Text
                    size="sm"
                    fw={500}
                    style={{ textTransform: "capitalize" }}
                  >
                    {p.provider}
                  </Text>
                  <Tooltip label={`Exact: ${label.exact}`} withArrow>
                    <Text
                      size="sm"
                      style={{ fontVariantNumeric: "tabular-nums" }}
                    >
                      {label.display}
                    </Text>
                  </Tooltip>
                </Group>
              );
            })}
          </div>
        ) : (
          <Text size="sm" c="dimmed">
            No cost events yet for this filter.
          </Text>
        )}
      </div>
    </div>
  );
}
