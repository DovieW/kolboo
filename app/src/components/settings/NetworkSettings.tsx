import {
  Button,
  Group,
  Loader,
  PasswordInput,
  SegmentedControl,
  Text,
  TextInput,
  Tooltip,
} from "@mantine/core";
import { useEffect, useMemo, useState } from "react";
import {
  useSettings,
  useSystemProxyInfo,
  useUpdateProxySettings,
} from "../../lib/queries";
import {
  tauriAPI,
  type ManualProxySettings,
  type ProxyMode,
  type ProxySettings,
} from "../../lib/tauri";

const GLOBAL_ONLY_TOOLTIP =
  "This setting can only be changed in the Default profile";

const defaultManualProxySettings: ManualProxySettings = {
  proxy_url: "",
  no_proxy: "localhost,127.0.0.1",
  username: "",
  password: "",
};

const defaultProxySettings: ProxySettings = {
  mode: "system",
  manual: defaultManualProxySettings,
};

export function NetworkSettings({
  editingProfileId,
}: {
  editingProfileId?: string;
}) {
  const isProfileScope = !!editingProfileId && editingProfileId !== "default";

  const { data: settings, isLoading: isLoadingSettings } = useSettings();
  const updateProxySettings = useUpdateProxySettings();

  const {
    data: systemProxyInfo,
    isLoading: isLoadingSystemProxyInfo,
    isFetching: isFetchingSystemProxyInfo,
  } = useSystemProxyInfo();

  const persisted = settings?.proxy_settings ?? defaultProxySettings;

  // Local draft state so we can let users select Manual mode before applying
  // (avoids breaking the pipeline when proxy_url is still empty).
  const [modeDraft, setModeDraft] = useState<ProxyMode>(persisted.mode);
  const [manualDraft, setManualDraft] = useState<ManualProxySettings>(
    persisted.manual
  );

  useEffect(() => {
    if (!settings) return;
    setModeDraft(settings.proxy_settings.mode);
    setManualDraft(settings.proxy_settings.manual);
  }, [
    settings?.proxy_settings.mode,
    settings?.proxy_settings.manual.proxy_url,
    settings?.proxy_settings.manual.no_proxy,
    settings?.proxy_settings.manual.username,
    settings?.proxy_settings.manual.password,
  ]);

  const canApplyManual = manualDraft.proxy_url.trim().length > 0;

  const effectiveModeLabel = useMemo(() => {
    if (persisted.mode === "no_proxy") return "No proxy";
    if (persisted.mode === "manual") return "Manual";
    return "System";
  }, [persisted.mode]);

  const persistProxySettings = (next: ProxySettings) => {
    updateProxySettings.mutate(next, {
      onSuccess: () => {
        tauriAPI.emitSettingsChanged();
      },
    });
  };

  const handleModeChange = (value: string) => {
    const nextMode = value as ProxyMode;
    setModeDraft(nextMode);

    if (isProfileScope) return;

    // System / No proxy can be applied immediately.
    if (nextMode === "system" || nextMode === "no_proxy") {
      persistProxySettings({ mode: nextMode, manual: manualDraft });
      return;
    }

    // Manual: only apply immediately if we already have a proxy URL.
    if (nextMode === "manual" && canApplyManual) {
      persistProxySettings({ mode: "manual", manual: manualDraft });
    }
  };

  const applyManual = () => {
    if (isProfileScope) return;
    if (!canApplyManual) return;
    persistProxySettings({ mode: "manual", manual: manualDraft });
  };

  const content = (
    <>
      <div className="settings-row">
        <div>
          <p className="settings-label">Proxy</p>
          <Text size="sm" c="dimmed">
            Control how Kolboo connects to the internet.
          </Text>
          <Text size="xs" c="dimmed" mt={6}>
            Effective mode: <strong>{effectiveModeLabel}</strong>
          </Text>
        </div>

        <div className="settings-row-actions" style={{ minWidth: 280 }}>
          <SegmentedControl
            value={modeDraft}
            onChange={handleModeChange}
            data={[
              { label: "No proxy", value: "no_proxy" },
              { label: "System", value: "system" },
              { label: "Manual", value: "manual" },
            ]}
            disabled={isLoadingSettings || isProfileScope}
          />
        </div>
      </div>

      {modeDraft === "no_proxy" && (
        <Text size="sm" c="dimmed" mt={10}>
          Disables all proxy usage, even if your OS or environment variables are
          configured to use one.
        </Text>
      )}

      {modeDraft === "system" && (
        <>
          <Text size="sm" c="dimmed" mt={10}>
            Uses your system/environment proxy configuration (Reqwest default
            behavior).
          </Text>

          <div style={{ marginTop: 12, display: "grid", gap: 10 }}>
            {isLoadingSystemProxyInfo || isFetchingSystemProxyInfo ? (
              <Group gap={8}>
                <Loader size="sm" color="orange" />
                <Text size="sm" c="dimmed">
                  Detecting system proxy settings…
                </Text>
              </Group>
            ) : (
              <>
                <TextInput
                  label="HTTP_PROXY (env)"
                  value={systemProxyInfo?.env_http_proxy ?? ""}
                  readOnly
                />
                <TextInput
                  label="HTTPS_PROXY (env)"
                  value={systemProxyInfo?.env_https_proxy ?? ""}
                  readOnly
                />
                <TextInput
                  label="NO_PROXY (env)"
                  value={systemProxyInfo?.env_no_proxy ?? ""}
                  readOnly
                />

                {systemProxyInfo?.windows_internet_settings && (
                  <>
                    <Text size="sm" c="dimmed" mt={6}>
                      Windows Internet Settings
                    </Text>
                    <TextInput
                      label="ProxyEnable"
                      value={
                        systemProxyInfo.windows_internet_settings
                          .proxy_enable === null
                          ? ""
                          : systemProxyInfo.windows_internet_settings
                              .proxy_enable
                          ? "1"
                          : "0"
                      }
                      readOnly
                    />
                    <TextInput
                      label="ProxyServer"
                      value={
                        systemProxyInfo.windows_internet_settings
                          .proxy_server ?? ""
                      }
                      readOnly
                    />
                    <TextInput
                      label="ProxyOverride"
                      value={
                        systemProxyInfo.windows_internet_settings
                          .proxy_override ?? ""
                      }
                      readOnly
                    />
                    <TextInput
                      label="AutoConfigURL"
                      value={
                        systemProxyInfo.windows_internet_settings
                          .auto_config_url ?? ""
                      }
                      readOnly
                    />
                  </>
                )}
              </>
            )}

            <Text size="xs" c="dimmed">
              Note: depending on your OS configuration (e.g. PAC scripts), the
              actual proxy used may not be directly visible here.
            </Text>
          </div>
        </>
      )}

      {modeDraft === "manual" && (
        <>
          <Text size="sm" c="dimmed" mt={10}>
            Sends all HTTP/HTTPS requests through a single proxy URL.
          </Text>

          <div style={{ marginTop: 12, display: "grid", gap: 10 }}>
            <TextInput
              label="Proxy URL"
              placeholder="http://127.0.0.1:8080"
              value={manualDraft.proxy_url}
              onChange={(e) =>
                setManualDraft((s) => ({ ...s, proxy_url: e.target.value }))
              }
              disabled={isProfileScope}
              error={
                modeDraft === "manual" && manualDraft.proxy_url.trim() === ""
                  ? "Required to enable Manual mode"
                  : undefined
              }
            />

            <TextInput
              label="No proxy / bypass list"
              description="Comma- or whitespace-separated (NO_PROXY semantics)."
              placeholder="localhost,127.0.0.1,*.internal"
              value={manualDraft.no_proxy}
              onChange={(e) =>
                setManualDraft((s) => ({ ...s, no_proxy: e.target.value }))
              }
              disabled={isProfileScope}
            />

            <Group grow align="flex-end">
              <TextInput
                label="Username (optional)"
                value={manualDraft.username}
                onChange={(e) =>
                  setManualDraft((s) => ({ ...s, username: e.target.value }))
                }
                disabled={isProfileScope}
              />
              <PasswordInput
                label="Password (optional)"
                value={manualDraft.password}
                onChange={(e) =>
                  setManualDraft((s) => ({ ...s, password: e.target.value }))
                }
                disabled={isProfileScope}
              />
            </Group>

            <Group justify="space-between" mt={4}>
              <Text size="xs" c="dimmed">
                Manual values are saved and restored when you switch modes.
              </Text>

              <Button
                size="xs"
                variant="filled"
                color="orange"
                onClick={applyManual}
                disabled={!canApplyManual || isProfileScope}
                loading={updateProxySettings.isPending}
              >
                Apply manual proxy
              </Button>
            </Group>

            {!canApplyManual && (
              <Text size="xs" c="dimmed">
                Enter a Proxy URL, then click “Apply manual proxy”.
              </Text>
            )}
          </div>
        </>
      )}
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
