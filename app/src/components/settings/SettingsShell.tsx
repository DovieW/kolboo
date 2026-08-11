import {
	ActionIcon,
	Select,
	type SelectProps,
	Tabs,
	Title,
	Tooltip,
} from "@mantine/core";
import { useQuery } from "@tanstack/react-query";
import { CircleHelp, Cog, Plus } from "lucide-react";
import { useEffect, useState } from "react";
import { API_KEY_STORE_KEYS } from "../../lib/apiKeys";
import { useLicenseAuthContext, useSettings } from "../../lib/queries";
import { hasManagedInferenceAccess, tauriAPI } from "../../lib/tauri";
import { ApiKeysSettings } from "./ApiKeysSettings";
import { AudioSettings } from "./AudioSettings";
import { DataSettings } from "./DataSettings";
import { HotkeySettings } from "./HotkeySettings";
import { NetworkSettings } from "./NetworkSettings";
import { PolicySettings } from "./PolicySettings";
import { PrivacySettings } from "./PrivacySettings";
import { ProfileConfigModal } from "./ProgramsModal";
import { PromptSettings } from "./PromptSettings";
import { UiSettings } from "./UiSettings";

export type SettingsShellProps = {
	onRunSetupGuide?: () => void;
};

type EditingOption = {
	value: string;
	label: string;
	isDisabledProfile?: boolean;
};

export function SettingsShell({ onRunSetupGuide }: SettingsShellProps) {
	const { data: settings } = useSettings();
	const { data: licenseAuthContext, isFetched: licenseAuthContextResolved } =
		useLicenseAuthContext();
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
		if (!licenseAuthContextResolved) return;

		const managedAccessEnabled = hasManagedInferenceAccess(licenseAuthContext);
		setActiveSettingsTab(
			hasAnyApiKey || managedAccessEnabled ? "ai" : "api-keys",
		);
	}, [
		hasAnyApiKey,
		hasUserSelectedTab,
		licenseAuthContext,
		licenseAuthContextResolved,
	]);

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
					<Title order={1}>Settings</Title>
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
						{onRunSetupGuide ? (
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
						) : null}

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
