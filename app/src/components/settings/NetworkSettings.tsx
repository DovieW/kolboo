import {
	ActionIcon,
	Button,
	Group,
	Loader,
	Modal,
	PasswordInput,
	SegmentedControl,
	Stack,
	Switch,
	Text,
	TextInput,
	Tooltip,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { open } from "@tauri-apps/plugin-dialog";
import { Trash2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import {
	useSaveProxySettings,
	useSettings,
	useSystemProxyInfo,
	useUpdateProxySettings,
} from "../../lib/queries";
import {
	type ManualProxySettings,
	type ProxyMode,
	type ProxySettings,
	type TrustedCaCertificate,
	tauriAPI,
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
	trusted_ca_certificates: [],
	danger_accept_invalid_certs: false,
};

export function NetworkSettings({
	editingProfileId,
}: {
	editingProfileId?: string;
}) {
	const isProfileScope = !!editingProfileId && editingProfileId !== "default";

	const { data: settings, isLoading: isLoadingSettings } = useSettings();
	const applyProxySettings = useUpdateProxySettings();
	const saveProxySettings = useSaveProxySettings();

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
		persisted.manual,
	);
	const [isAddingCert, setIsAddingCert] = useState(false);
	const [clearCertsDialogOpen, setClearCertsDialogOpen] = useState(false);
	const [isClearingCerts, setIsClearingCerts] = useState(false);

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

	const trustedCertNames = useMemo(() => {
		const certs = persisted.trusted_ca_certificates ?? [];
		return certs.map((c) => c.file_name || "(certificate)");
	}, [persisted.trusted_ca_certificates]);

	const effectiveModeLabel = useMemo(() => {
		if (persisted.mode === "no_proxy") return "No proxy";
		if (persisted.mode === "manual") return "Manual";

		// System mode precedence (reqwest/hyper-util): env vars first, then OS.
		// Reflect the *current* effective source in the label.
		if (isLoadingSystemProxyInfo || isFetchingSystemProxyInfo) {
			return "System (detecting…)";
		}

		const hasEnvProxy =
			!!systemProxyInfo?.env_http_proxy || !!systemProxyInfo?.env_https_proxy;
		return hasEnvProxy
			? "System (environment variables)"
			: "System (internet settings)";
	}, [
		persisted.mode,
		isLoadingSystemProxyInfo,
		isFetchingSystemProxyInfo,
		systemProxyInfo?.env_http_proxy,
		systemProxyInfo?.env_https_proxy,
	]);

	const persistProxySettings = (
		next: ProxySettings,
		opts?: { onSuccess?: () => void },
	) => {
		applyProxySettings.mutate(next, {
			onSuccess: () => {
				tauriAPI.emitSettingsChanged();
				opts?.onSuccess?.();
			},
		});
	};

	const saveProxySettingsDraft = (next: ProxySettings) => {
		saveProxySettings.mutate(next, {
			onSuccess: () => {
				tauriAPI.emitSettingsChanged();
			},
		});
	};

	// Save manual edits; if Manual can be enabled (proxy URL provided), apply it.
	const saveManualDraft = (opts: { enableIfPossible: boolean }) => {
		if (isProfileScope) return;

		// Never persist/apply an invalid manual proxy URL. Users can switch away
		// from Manual mode first if they want to clear it.
		if (modeDraft === "manual" && !canApplyManual) {
			return;
		}

		const shouldEnableManual =
			opts.enableIfPossible && modeDraft === "manual" && canApplyManual;

		if (persisted.mode === "manual" || shouldEnableManual) {
			persistProxySettings({
				mode: "manual",
				manual: manualDraft,
				trusted_ca_certificates: persisted.trusted_ca_certificates ?? [],
				danger_accept_invalid_certs: persisted.danger_accept_invalid_certs,
			});
			return;
		}

		// Save the manual fields for later, but keep the effective mode unchanged.
		saveProxySettingsDraft({
			mode: persisted.mode,
			manual: manualDraft,
			trusted_ca_certificates: persisted.trusted_ca_certificates ?? [],
			danger_accept_invalid_certs: persisted.danger_accept_invalid_certs,
		});
	};

	const handleModeChange = (value: string) => {
		const nextMode = value as ProxyMode;
		setModeDraft(nextMode);

		if (isProfileScope) return;

		// System / No proxy can be applied immediately.
		if (nextMode === "system" || nextMode === "no_proxy") {
			persistProxySettings({
				mode: nextMode,
				manual: manualDraft,
				trusted_ca_certificates: persisted.trusted_ca_certificates ?? [],
				danger_accept_invalid_certs: persisted.danger_accept_invalid_certs,
			});
			return;
		}

		// Manual: only apply immediately if we already have a proxy URL.
		if (nextMode === "manual" && canApplyManual) {
			persistProxySettings({
				mode: "manual",
				manual: manualDraft,
				trusted_ca_certificates: persisted.trusted_ca_certificates ?? [],
				danger_accept_invalid_certs: persisted.danger_accept_invalid_certs,
			});
			return;
		}

		// Otherwise, keep draft mode as Manual (so the fields show), but don't
		// change the effective mode until a proxy URL is provided.
		if (nextMode === "manual") {
			saveManualDraft({ enableIfPossible: false });
		}
	};

	const content = (
		<>
			<div className="settings-row">
				<div>
					<p className="settings-label">Add certificate</p>
					<p className="settings-description">
						Add a CA certificate to trust for HTTPS requests
					</p>
				</div>

				<div
					className="settings-row-actions"
					style={{ minWidth: 280, justifyContent: "flex-end" }}
				>
					<Group gap="xs" wrap="nowrap" justify="flex-end">
						<Tooltip label="Remove all saved certificates" withArrow>
							<ActionIcon
								variant="subtle"
								color="red"
								size="lg"
								disabled={
									isLoadingSettings ||
									isProfileScope ||
									(persisted.trusted_ca_certificates?.length ?? 0) === 0
								}
								onClick={() => {
									if (isProfileScope) return;
									if ((persisted.trusted_ca_certificates?.length ?? 0) === 0)
										return;
									setClearCertsDialogOpen(true);
								}}
								aria-label="Remove all certificates"
							>
								<Trash2 size={16} />
							</ActionIcon>
						</Tooltip>

						<Button
							variant="default"
							onClick={async () => {
								if (isProfileScope || isAddingCert) return;
								setIsAddingCert(true);

								try {
									const selected = await open({
										title: "Select CA certificate(s)",
										multiple: true,
										filters: [
											{
												name: "Certificates",
												extensions: ["pem", "crt", "cer", "der"],
											},
											{ name: "All files", extensions: ["*"] },
										],
									});

									if (!selected) return;

									const selectedPaths = Array.isArray(selected)
										? selected
										: [selected];

									if (selectedPaths.length === 0) return;

									const existing = persisted.trusted_ca_certificates ?? [];
									let nextTrusted: TrustedCaCertificate[] = [...existing];
									let added = 0;
									let duplicates = 0;
									const failed: Array<{ file: string; error: string }> = [];
									const addedNames: string[] = [];

									for (const path of selectedPaths) {
										try {
											const cert =
												await tauriAPI.loadTrustedCaCertificateFromFile(path);

											const alreadyHave = nextTrusted.some(
												(c) =>
													c.data_base64 === cert.data_base64 &&
													c.format === cert.format,
											);

											if (alreadyHave) {
												duplicates += 1;
												continue;
											}

											nextTrusted = [...nextTrusted, cert];
											added += 1;
											addedNames.push(cert.file_name || "(certificate)");
										} catch (e) {
											failed.push({ file: path, error: String(e) });
										}
									}

									if (added === 0) {
										if (failed.length > 0) {
											notifications.show({
												title: "Failed to add certificates",
												message:
													failed.length === 1
														? (failed.at(0)?.error ?? "Failed to add.")
														: `${failed.length} certificates failed to add.`,
												color: "red",
											});
										} else if (duplicates > 0) {
											notifications.show({
												title: "No new certificates",
												message:
													duplicates === 1
														? "That certificate is already added."
														: "All selected certificates were already added.",
												color: "gray",
											});
										}
										return;
									}

									persistProxySettings(
										{
											mode: persisted.mode,
											manual:
												persisted.mode === "manual"
													? persisted.manual
													: manualDraft,
											trusted_ca_certificates: nextTrusted,
											danger_accept_invalid_certs:
												persisted.danger_accept_invalid_certs,
										},
										{
											onSuccess: () => {
												const summaryParts: string[] = [];
												summaryParts.push(
													`${added} added${
														duplicates > 0 ? `, ${duplicates} duplicate` : ""
													}${
														failed.length > 0 ? `, ${failed.length} failed` : ""
													}`,
												);

												const namePreview = addedNames.slice(0, 3).join(", ");
												const message =
													addedNames.length <= 3
														? namePreview
														: `${namePreview} (+${addedNames.length - 3} more)`;

												notifications.show({
													title:
														added === 1
															? "Certificate added"
															: "Certificates added",
													message: message || summaryParts.join(" "),
													color: failed.length > 0 ? "yellow" : "green",
												});

												if (failed.length > 0) {
													const failedPreview = failed
														.slice(0, 2)
														.map((f) => f.file)
														.join(", ");
													notifications.show({
														title: "Some certificates failed",
														message:
															failed.length <= 2
																? failedPreview
																: `${failedPreview} (+${
																		failed.length - 2
																	} more)`,
														color: "red",
													});
												}
											},
										},
									);
								} catch (error) {
									notifications.show({
										title: "Failed to add certificate",
										message: String(error),
										color: "red",
									});
								} finally {
									setIsAddingCert(false);
								}
							}}
							disabled={isLoadingSettings || isProfileScope || isAddingCert}
							loading={isAddingCert}
						>
							Add certificate
						</Button>
					</Group>
				</div>
			</div>

			{(persisted.trusted_ca_certificates?.length ?? 0) > 0 && (
				<div style={{ marginTop: 8, display: "grid", gap: 6 }}>
					{persisted.trusted_ca_certificates.map((c) => (
						<div
							key={c.id}
							style={{
								display: "flex",
								alignItems: "center",
								justifyContent: "space-between",
								gap: 12,
							}}
						>
							<Text size="sm" c="dimmed" style={{ overflow: "hidden" }}>
								{c.file_name || "(certificate)"}
							</Text>
							<Button
								variant="subtle"
								color="gray"
								size="xs"
								onClick={() => {
									if (isProfileScope) return;
									const next = (persisted.trusted_ca_certificates ?? []).filter(
										(x) => x.id !== c.id,
									);
									persistProxySettings(
										{
											mode: persisted.mode,
											manual:
												persisted.mode === "manual"
													? persisted.manual
													: manualDraft,
											trusted_ca_certificates: next,
											danger_accept_invalid_certs:
												persisted.danger_accept_invalid_certs,
										},
										{
											onSuccess: () => {
												notifications.show({
													title: "Certificate removed",
													message: c.file_name || "Removed",
													color: "gray",
												});
											},
										},
									);
								}}
								disabled={isLoadingSettings || isProfileScope}
							>
								Remove
							</Button>
						</div>
					))}
				</div>
			)}

			<div className="settings-row">
				<div>
					<p className="settings-label">Ignore invalid certifications</p>
					<p className="settings-description">
						Allow connections even when certificates are invalid (e.g.
						self-signed)
					</p>
				</div>

				<div style={{ display: "flex", alignItems: "center", gap: 8 }}>
					<Switch
						checked={persisted.danger_accept_invalid_certs}
						onChange={(e) => {
							if (isProfileScope) return;

							const enabled = e.currentTarget.checked;
							// Apply immediately, but avoid accidentally enabling invalid
							// Manual mode while the user is drafting.
							persistProxySettings({
								mode: persisted.mode,
								manual:
									persisted.mode === "manual" ? persisted.manual : manualDraft,
								trusted_ca_certificates:
									persisted.trusted_ca_certificates ?? [],
								danger_accept_invalid_certs: enabled,
							});
						}}
						disabled={isLoadingSettings || isProfileScope}
						color="gray"
						size="md"
					/>
				</div>
			</div>

			<Modal
				opened={clearCertsDialogOpen}
				onClose={() => {
					if (isClearingCerts) return;
					setClearCertsDialogOpen(false);
				}}
				title="Remove all certificates"
				centered
				size="sm"
			>
				<Stack gap="sm">
					<Text size="sm">
						This will remove all trusted CA certificates you added.
					</Text>
					<Text size="xs" c="dimmed">
						Currently saved: <strong>{trustedCertNames.length}</strong>
					</Text>

					{trustedCertNames.length > 0 && (
						<div style={{ maxHeight: 160, overflow: "auto" }}>
							<Stack gap={4}>
								{trustedCertNames.slice(0, 10).map((name) => (
									<Text key={name} size="xs" c="dimmed">
										• {name}
									</Text>
								))}
								{trustedCertNames.length > 10 && (
									<Text size="xs" c="dimmed">
										…and {trustedCertNames.length - 10} more
									</Text>
								)}
							</Stack>
						</div>
					)}

					<Group justify="flex-end" gap="sm">
						<Button
							variant="default"
							disabled={isClearingCerts}
							onClick={() => setClearCertsDialogOpen(false)}
						>
							Cancel
						</Button>
						<Button
							color="red"
							loading={isClearingCerts}
							onClick={async () => {
								if (isProfileScope) return;
								if (trustedCertNames.length === 0) {
									setClearCertsDialogOpen(false);
									return;
								}

								try {
									setIsClearingCerts(true);
									persistProxySettings(
										{
											mode: persisted.mode,
											manual:
												persisted.mode === "manual"
													? persisted.manual
													: manualDraft,
											trusted_ca_certificates: [],
											danger_accept_invalid_certs:
												persisted.danger_accept_invalid_certs,
										},
										{
											onSuccess: () => {
												notifications.show({
													title: "Certificates removed",
													message: `Removed ${trustedCertNames.length}`,
													color: "gray",
												});
											},
										},
									);
									setClearCertsDialogOpen(false);
								} catch (e) {
									notifications.show({
										title: "Failed to remove certificates",
										message: String(e),
										color: "red",
									});
								} finally {
									setIsClearingCerts(false);
								}
							}}
						>
							Remove all
						</Button>
					</Group>
				</Stack>
			</Modal>

			<div className="settings-row no-divider">
				<div>
					<p className="settings-label">Proxy</p>
					<Text size="xs" c="dimmed" mt={6}>
						Effective mode: <strong>{effectiveModeLabel}</strong>
					</Text>
				</div>

				<div
					className="settings-row-actions"
					style={{ minWidth: 280, justifyContent: "flex-end" }}
				>
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
							<Text size="sm" c="dimmed" mt={6}>
								Environment Variables
							</Text>
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
											systemProxyInfo.windows_internet_settings.proxy_enable ===
											null
												? ""
												: systemProxyInfo.windows_internet_settings.proxy_enable
													? "1"
													: "0"
										}
										readOnly
									/>
									<TextInput
										label="ProxyServer"
										value={
											systemProxyInfo.windows_internet_settings.proxy_server ??
											""
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

					{!!systemProxyInfo &&
						(systemProxyInfo.env_http_proxy ||
							systemProxyInfo.env_https_proxy ||
							systemProxyInfo.env_no_proxy) &&
						!!systemProxyInfo.windows_internet_settings && (
							<Text size="xs" c="dimmed">
								Both environment variables and Windows proxy settings are
								present. In System mode, env vars usually “win”.
							</Text>
						)}

					<Text size="xs" c="dimmed">
						Note: depending on your OS configuration (e.g. PAC scripts), the
						actual proxy used may not be directly visible here.
					</Text>
				</div>
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
							onBlur={() => {
								// If the user has chosen Manual, enable it as soon as a valid
								// proxy URL exists.
								saveManualDraft({ enableIfPossible: true });
							}}
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
							onBlur={() => {
								// If Manual is active, apply changes; otherwise save for later.
								saveManualDraft({ enableIfPossible: true });
							}}
							disabled={isProfileScope}
						/>

						<Group grow align="flex-end">
							<TextInput
								label="Username (optional)"
								value={manualDraft.username}
								onChange={(e) =>
									setManualDraft((s) => ({ ...s, username: e.target.value }))
								}
								onBlur={() => {
									saveManualDraft({ enableIfPossible: true });
								}}
								disabled={isProfileScope}
							/>
							<PasswordInput
								label="Password (optional)"
								value={manualDraft.password}
								onChange={(e) =>
									setManualDraft((s) => ({ ...s, password: e.target.value }))
								}
								onBlur={() => {
									saveManualDraft({ enableIfPossible: true });
								}}
								disabled={isProfileScope}
							/>
						</Group>
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
