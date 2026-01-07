import {
  ActionIcon,
  Button,
  Group,
  PasswordInput,
  TextInput,
  Switch,
  Text,
  Tooltip,
} from "@mantine/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { useEffect, useRef, useState } from "react";
import { Link as LinkIcon } from "lucide-react";
import { configAPI, tauriAPI } from "../../lib/tauri";
import {
  EMBEDDING_MODELS,
  LLM_MODELS,
  STT_MODELS,
} from "../../lib/modelOptions";
import {
  useSettings,
  useUpdateAssemblyAiFreeTier,
  useUpdateCerebrasFreeTier,
  useUpdateCohereFreeTier,
  useUpdateGroqFreeTier,
  useUpdateSpeechmaticsFreeTier,
  useUpdateWhisperServerBaseUrl,
} from "../../lib/queries";

const GLOBAL_ONLY_TOOLTIP =
  "This setting can only be changed in the Default profile";

interface ApiKeyConfig {
  id: string;
  label: string;
  placeholder: string;
  storeKey: string;
  getKeyUrl: string;
}

const API_KEYS: ApiKeyConfig[] = [
  {
    id: "groq",
    label: "Groq",
    placeholder: "Enter API key",
    storeKey: "groq_api_key",
    getKeyUrl: "https://console.groq.com/keys",
  },
  {
    id: "cerebras",
    label: "Cerebras",
    placeholder: "Enter API key",
    storeKey: "cerebras_api_key",
    getKeyUrl: "https://cloud.cerebras.ai/platform",
  },
  {
    id: "cohere",
    label: "Cohere",
    placeholder: "Enter API key",
    storeKey: "cohere_api_key",
    getKeyUrl: "https://dashboard.cohere.com/api-keys",
  },
  {
    id: "assemblyai",
    label: "AssemblyAI",
    placeholder: "Enter API key",
    storeKey: "assemblyai_api_key",
    getKeyUrl: "https://www.assemblyai.com/dashboard/api-keys",
  },
  {
    id: "speechmatics",
    label: "Speechmatics",
    placeholder: "Enter API key",
    storeKey: "speechmatics_api_key",
    getKeyUrl: "https://portal.speechmatics.com/settings/api-keys",
  },
  {
    id: "aquavoice",
    label: "Aquavoice (Avalon)",
    placeholder: "Enter API key",
    storeKey: "aquavoice_api_key",
    getKeyUrl: "https://app.aquavoice.com/api-dashboard?tab=keys",
  },
  {
    id: "gemini",
    label: "Google AI Studio",
    placeholder: "Enter API key",
    storeKey: "gemini_api_key",
    getKeyUrl: "https://aistudio.google.com/apikey",
  },
  {
    id: "openai",
    label: "OpenAI",
    placeholder: "Enter API key",
    storeKey: "openai_api_key",
    getKeyUrl: "https://platform.openai.com/api-keys",
  },
  {
    id: "deepgram",
    label: "Deepgram",
    placeholder: "Enter API key",
    storeKey: "deepgram_api_key",
    getKeyUrl: "https://console.deepgram.com/project",
  },
  {
    id: "anthropic",
    label: "Anthropic",
    placeholder: "Enter API key",
    storeKey: "anthropic_api_key",
    getKeyUrl: "https://platform.claude.com/settings/keys",
  },
];

export const API_KEY_STORE_KEYS = API_KEYS.map((k) => k.storeKey);

function formatProviderModelCounts(providerId: string): string | null {
  const sttCount = STT_MODELS[providerId]?.length ?? 0;
  const llmCount = LLM_MODELS[providerId]?.length ?? 0;
  const embedCount = EMBEDDING_MODELS[providerId]?.length ?? 0;

  const parts: string[] = [];
  if (sttCount > 0) parts.push(`${sttCount} STT`);
  if (embedCount > 0) parts.push(`${embedCount} Embed`);
  if (llmCount > 0) parts.push(`${llmCount} LLM`);
  if (parts.length === 0) return null;
  return parts.join(" / ");
}

function formatProviderModelsTooltip(providerId: string): ReactNode | null {
  const embed = EMBEDDING_MODELS[providerId] ?? [];
  const stt = STT_MODELS[providerId] ?? [];
  const llm = LLM_MODELS[providerId] ?? [];

  if (embed.length === 0 && stt.length === 0 && llm.length === 0) return null;

  const formatList = (items: Array<{ label: string; value: string }>) =>
    items.map((m) => m.label || m.value).join(", ");

  return (
    <div style={{ maxWidth: 420 }}>
      {embed.length > 0 ? (
        <Text size="xs" fw={600}>
          Embed
        </Text>
      ) : null}
      {embed.length > 0 ? (
        <Text size="xs" c="dimmed" style={{ lineHeight: 1.35 }}>
          {formatList(embed)}
        </Text>
      ) : null}

      {stt.length > 0 ? (
        <Text size="xs" fw={600} mt={embed.length > 0 ? 8 : 0}>
          STT
        </Text>
      ) : null}
      {stt.length > 0 ? (
        <Text size="xs" c="dimmed" style={{ lineHeight: 1.35 }}>
          {formatList(stt)}
        </Text>
      ) : null}

      {llm.length > 0 ? (
        <Text
          size="xs"
          fw={600}
          mt={embed.length > 0 || stt.length > 0 ? 8 : 0}
        >
          LLM
        </Text>
      ) : null}
      {llm.length > 0 ? (
        <Text size="xs" c="dimmed" style={{ lineHeight: 1.35 }}>
          {formatList(llm)}
        </Text>
      ) : null}
    </div>
  );
}

function ApiKeyInput({ config }: { config: ApiKeyConfig }) {
  const queryClient = useQueryClient();
  const [value, setValue] = useState("");
  const [isPrefilling, setIsPrefilling] = useState(false);
  const hasHydratedRef = useRef(false);

  const { data: settings } = useSettings();
  const updateGroqFreeTier = useUpdateGroqFreeTier();
  const updateCerebrasFreeTier = useUpdateCerebrasFreeTier();
  const updateAssemblyAiFreeTier = useUpdateAssemblyAiFreeTier();
  const updateSpeechmaticsFreeTier = useUpdateSpeechmaticsFreeTier();
  const updateCohereFreeTier = useUpdateCohereFreeTier();

  const { data: savedKeyValue } = useQuery({
    queryKey: ["apiKeyValue", config.storeKey],
    queryFn: () => tauriAPI.getApiKey(config.storeKey),
    staleTime: 0,
  });

  useEffect(() => {
    if (hasHydratedRef.current) return;
    if (!savedKeyValue) return;

    // Mirror the setup guide: if a key exists, show it in the PasswordInput
    // (hidden by default), so Show/Hide reveals something useful.
    setValue(savedKeyValue);
    hasHydratedRef.current = true;
  }, [savedKeyValue]);

  const saveKey = useMutation({
    mutationFn: async (key: string) => {
      await tauriAPI.setApiKey(config.storeKey, key);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({
        queryKey: ["apiKeyValue", config.storeKey],
      });
      queryClient.invalidateQueries({ queryKey: ["availableProviders"] });
      // Sync pipeline config when API keys change
      configAPI.syncPipelineConfig();
      // Keep the saved value in the field so the button disables when unchanged.
      setValue((prev) => prev.trim());
      hasHydratedRef.current = true;
    },
  });

  const handleSave = () => {
    const trimmed = value.trim();
    if (!trimmed) return;
    saveKey.mutate(trimmed);
  };

  const trimmedValue = value.trim();
  const trimmedSaved = (savedKeyValue ?? "").trim();
  const isUnchanged =
    trimmedSaved.length > 0 &&
    trimmedValue.length > 0 &&
    trimmedValue === trimmedSaved;

  const modelCountsLabel = formatProviderModelCounts(config.id);
  const modelsTooltip = formatProviderModelsTooltip(config.id);

  return (
    <div className="settings-row api-keys-row">
      <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
        <p className="settings-label">{config.label}</p>
        {config.id === "groq" && (
          <Group gap={10} align="center" wrap="nowrap" mt={2}>
            <Switch
              size="sm"
              checked={settings?.groq_free_tier ?? true}
              onChange={(e) =>
                updateGroqFreeTier.mutate(e.currentTarget.checked)
              }
              aria-label="Groq free tier"
            />
            <Text size="xs" c="var(--text-secondary)" fw={600}>
              Free tier
            </Text>
            <Text
              size="xs"
              c="var(--text-muted)"
              className="settings-description--single-line"
              style={{ flex: 1 }}
              title="Assume Groq calls cost $0 for stats"
            >
              Assume Groq calls cost $0 for stats
            </Text>
          </Group>
        )}
        {config.id === "cerebras" && (
          <Group gap={10} align="center" wrap="nowrap" mt={2}>
            <Switch
              size="sm"
              checked={settings?.cerebras_free_tier ?? true}
              onChange={(e) =>
                updateCerebrasFreeTier.mutate(e.currentTarget.checked)
              }
              aria-label="Cerebras free tier"
            />
            <Text size="xs" c="var(--text-secondary)" fw={600}>
              Free tier
            </Text>
            <Text
              size="xs"
              c="var(--text-muted)"
              className="settings-description--single-line"
              style={{ flex: 1 }}
              title="Assume Cerebras calls cost $0 for stats"
            >
              Assume Cerebras calls cost $0 for stats
            </Text>
          </Group>
        )}
        {config.id === "assemblyai" && (
          <Group gap={10} align="center" wrap="nowrap" mt={2}>
            <Switch
              size="sm"
              checked={settings?.assemblyai_free_tier ?? true}
              onChange={(e) =>
                updateAssemblyAiFreeTier.mutate(e.currentTarget.checked)
              }
              aria-label="AssemblyAI free tier"
            />
            <Text size="xs" c="var(--text-secondary)" fw={600}>
              Free tier
            </Text>
            <Text
              size="xs"
              c="var(--text-muted)"
              className="settings-description--single-line"
              style={{ flex: 1 }}
              title="Assume AssemblyAI calls cost $0 for stats"
            >
              Assume AssemblyAI calls cost $0 for stats
            </Text>
          </Group>
        )}
        {config.id === "speechmatics" && (
          <Group gap={10} align="center" wrap="nowrap" mt={2}>
            <Switch
              size="sm"
              checked={settings?.speechmatics_free_tier ?? true}
              onChange={(e) =>
                updateSpeechmaticsFreeTier.mutate(e.currentTarget.checked)
              }
              aria-label="Speechmatics free tier"
            />
            <Text size="xs" c="var(--text-secondary)" fw={600}>
              Free tier
            </Text>
            <Text
              size="xs"
              c="var(--text-muted)"
              className="settings-description--single-line"
              style={{ flex: 1 }}
              title="Assume Speechmatics calls cost $0 for stats"
            >
              Assume Speechmatics calls cost $0 for stats
            </Text>
          </Group>
        )}
        {config.id === "cohere" && (
          <Group gap={10} align="center" wrap="nowrap" mt={2}>
            <Switch
              size="sm"
              checked={settings?.cohere_free_tier ?? true}
              onChange={(e) =>
                updateCohereFreeTier.mutate(e.currentTarget.checked)
              }
              aria-label="Cohere free tier"
            />
            <Text size="xs" c="var(--text-secondary)" fw={600}>
              Free tier
            </Text>
            <Text
              size="xs"
              c="var(--text-muted)"
              className="settings-description--single-line"
              style={{ flex: 1 }}
              title="Assume Cohere calls cost $0 for stats"
            >
              Assume Cohere calls cost $0 for stats
            </Text>
          </Group>
        )}
      </div>
      <div className="settings-row-actions">
        {modelCountsLabel && (
          <Tooltip
            label={modelsTooltip ?? ""}
            withArrow
            multiline
            disabled={!modelsTooltip}
            position="bottom"
            styles={{
              tooltip: {
                backgroundColor: "var(--bg-elevated)",
                color: "var(--text-primary)",
                border: "1px solid var(--border-default)",
              },
            }}
          >
            <Text
              size="xs"
              c="var(--text-muted)"
              style={{ alignSelf: "center", whiteSpace: "nowrap" }}
            >
              {modelCountsLabel}
            </Text>
          </Tooltip>
        )}
        <Tooltip label="Get key" withArrow>
          <ActionIcon
            component="a"
            href={config.getKeyUrl}
            target="_blank"
            rel="noreferrer"
            variant="subtle"
            color="gray"
            size={36}
          >
            <LinkIcon size={16} />
          </ActionIcon>
        </Tooltip>
        <PasswordInput
          value={value}
          onChange={(e) => setValue(e.currentTarget.value)}
          placeholder={config.placeholder}
          size="sm"
          disabled={isPrefilling || saveKey.isPending}
          styles={{
            input: {
              backgroundColor: "var(--bg-elevated)",
              borderColor: "var(--border-default)",
              color: "var(--text-primary)",
              height: 36,
              width: 200,
            },
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") handleSave();
          }}
        />
        <Tooltip label="Set API key">
          <Button
            color="orange"
            size="sm"
            onClick={handleSave}
            loading={saveKey.isPending}
            disabled={!trimmedValue || saveKey.isPending || isUnchanged}
            styles={{
              root: {
                height: 36,
              },
            }}
          >
            Set
          </Button>
        </Tooltip>
      </div>
    </div>
  );
}

export function ApiKeysSettings({
  editingProfileId,
}: {
  editingProfileId?: string;
}) {
  const isProfileScope = editingProfileId && editingProfileId !== "default";

  const { data: settings } = useSettings();
  const updateWhisperServerBaseUrl = useUpdateWhisperServerBaseUrl();
  const [whisperServerBaseUrlDraft, setWhisperServerBaseUrlDraft] = useState(
    settings?.whisper_server_base_url ?? ""
  );

  useEffect(() => {
    setWhisperServerBaseUrlDraft(settings?.whisper_server_base_url ?? "");
  }, [settings?.whisper_server_base_url]);

  const content = (
    <>
      {API_KEYS.map((config) => (
        <ApiKeyInput key={config.id} config={config} />
      ))}

      <div className="settings-row">
        <div>
          <p className="settings-label">Whisper server URL</p>
          <p className="settings-description">
            Base URL for an OpenAI-compatible transcription API (e.g.
            http://localhost:8000/v1)
          </p>
        </div>
        <TextInput
          value={whisperServerBaseUrlDraft}
          onChange={(e) =>
            setWhisperServerBaseUrlDraft(e.currentTarget.value)
          }
          onBlur={() => {
            const trimmed = whisperServerBaseUrlDraft.trim();
            const normalized = trimmed ? trimmed : null;
            updateWhisperServerBaseUrl.mutate(normalized, {
              onSuccess: () => {
                tauriAPI.emitSettingsChanged();
              },
            });
          }}
          placeholder="http://localhost:8000/v1"
          styles={{
            input: {
              backgroundColor: "var(--bg-elevated)",
              borderColor: "var(--border-default)",
              color: "var(--text-primary)",
              minWidth: 280,
            },
          }}
        />
      </div>
    </>
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
