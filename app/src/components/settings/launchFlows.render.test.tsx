import { MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";

const { mutation, query } = vi.hoisted(() => ({
	mutation: () => ({
		isPending: false,
		mutate: vi.fn(),
		mutateAsync: vi.fn(),
	}),
	query: (data: unknown) => ({
		data,
		isError: false,
		isLoading: false,
		isPending: false,
	}),
}));

vi.mock("../../lib/queries", () => ({
	useByokLlmModels: () => query([]),
	useSettings: () =>
		query({
			groq_free_tier: true,
			cerebras_free_tier: true,
			cohere_free_tier: true,
			assemblyai_free_tier: true,
			speechmatics_free_tier: true,
		}),
	useCancelWhisperModelDownload: mutation,
	useDeleteWhisperModel: mutation,
	useDownloadWhisperModel: mutation,
	useIsLocalWhisperAvailable: () => query(false),
	useIsLocalWhisperModelLoaded: () => query(false),
	useLoadLocalWhisperModel: mutation,
	useLocalWhisperBackendStatus: () => query(null),
	useUnloadLocalWhisperModel: mutation,
	useUpdateAssemblyAiFreeTier: mutation,
	useUpdateCerebrasFreeTier: mutation,
	useUpdateCohereFreeTier: mutation,
	useUpdateGroqFreeTier: mutation,
	useUpdateLocalWhisperLoadMode: mutation,
	useUpdateLocalWhisperModelId: mutation,
	useUpdateOcrAuthMode: mutation,
	useUpdateOcrAutoCaptureTiming: mutation,
	useUpdateOcrBaseUrl: mutation,
	useUpdateOcrContextMaxChars: mutation,
	useUpdateOcrHallucinationProtection: mutation,
	useUpdateOcrHallucinationThreshold: mutation,
	useUpdateOcrMaxTokens: mutation,
	useUpdateOcrModel: mutation,
	useUpdateOcrPrompt: mutation,
	useUpdateOcrRequestTimeoutMs: mutation,
	useUpdateOcrResizeFilter: mutation,
	useUpdateOcrResizeMaxDimension: mutation,
	useUpdateOcrTemperature: mutation,
	useUpdateOcrTopP: mutation,
	useUpdateOllamaUrl: mutation,
	useUpdateSpeechmaticsFreeTier: mutation,
	useUpdateWhisperServerBaseUrl: mutation,
	useValidateWhisperModel: mutation,
	useWhisperModels: () => query([]),
	useWhisperModelsDir: () => query(null),
}));

import { ApiKeysSettings } from "./ApiKeysSettings";
import { DataCloudSyncSection } from "./data/DataCloudSyncSection";
import { TelemetryDisclosureContent } from "./TelemetryDisclosureModal";

function render(ui: ReactNode): string {
	return renderToStaticMarkup(
		<MantineProvider>
			<QueryClientProvider client={new QueryClient()}>{ui}</QueryClientProvider>
		</MantineProvider>,
	);
}

describe("launch-critical rendered settings flows", () => {
	it("renders API-key management as a Community/BYOK capability", () => {
		const html = render(<ApiKeysSettings />);

		expect(html).toContain("OpenAI");
		expect(html).toContain("Groq");
		expect(html).toContain("Your key, stored securely on this device.");
		expect(html).toContain("Paste your API key");
	});

	it("keeps analytics paused behind an explicit disclosure", () => {
		const html = render(
			<TelemetryDisclosureContent
				analyticsPolicyEnforced={false}
				analyticsPolicyReason={null}
				loading={false}
				onDisableAnalytics={vi.fn()}
				onAllowAnalytics={vi.fn()}
			/>,
		);

		expect(html).toContain("basic usage without a persistent ID");
		expect(html).toContain("No audio or transcript content is sent");
		expect(html).toContain("Basic usage only");
		expect(html).toContain("What’s shared?");
		expect(html).not.toContain("Recording events include rounded duration");
		const optionalHtml = render(
			<TelemetryDisclosureContent
				analyticsPolicyEnforced={false}
				analyticsPolicyReason={null}
				loading={false}
				onDisableAnalytics={vi.fn()}
				onAllowAnalytics={vi.fn()}
			/>,
		);
		expect(optionalHtml).toContain("Basic usage only");
		expect(optionalHtml).toContain("Allow linked analytics");

		const policyHtml = render(
			<TelemetryDisclosureContent
				analyticsPolicyEnforced
				analyticsPolicyReason="Workspace policy"
				loading={false}
				onDisableAnalytics={vi.fn()}
				onAllowAnalytics={vi.fn()}
			/>,
		);
		expect(policyHtml).toContain("No events will be sent");
		expect(policyHtml).toContain("Continue");
		expect(policyHtml).not.toContain("Allow linked analytics");
	});

	it("makes global provider controls inert for non-default profiles, including keyboard input", () => {
		expect(render(<ApiKeysSettings editingProfileId="work" />)).toContain(
			'inert=""',
		);
	});

	it("renders managed cloud sync as blocked for Community/BYOK", () => {
		const html = render(
			<DataCloudSyncSection
				isProfileScope={false}
				globalOnlyTooltip="Default profile only"
				analyticsPolicyEnforced={false}
				analyticsPolicyReason={null}
				cloudSyncState={null}
				cloudSyncStateLoading={false}
				runCloudSyncActionPending={false}
				cloudSyncAccess={{
					status: "upgrade_required",
					canUseCloudSync: false,
					badgeColor: "orange",
					badgeLabel: "Upgrade required",
					helperLabel:
						"Community/BYOK remains available. Upgrade to use managed settings sync.",
				}}
				onPushCloudSync={vi.fn()}
				onPullCloudSync={vi.fn()}
				updateCloudSyncEnabledPending={false}
				onCloudSyncEnabledChange={vi.fn()}
				updateCloudSyncAutoPushPending={false}
				onCloudSyncAutoPushChange={vi.fn()}
				updatePosthogAnalyticsEnabledPending={false}
				onPosthogAnalyticsEnabledChange={vi.fn()}
			/>,
		);

		expect(html).toContain("Community/BYOK");
		expect(html).toContain(
			"Community/BYOK remains available. Upgrade to use managed settings sync.",
		);
		expect(html).toContain("Cloud sync (Personal+)");
		expect(html).toContain("First-run disclosure pending");
	});
});
