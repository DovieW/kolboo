import type { LicenseState } from "../../lib/tauri";

// Keep the first-run guide order centralized here so the setup copy/tests can
// assert that account setup stays ahead of provider configuration.
export const SETTINGS_GUIDE_STEPS = [
	"account",
	"groq",
	"dictation",
	"wrapup",
] as const;

export type SettingsGuideStep = (typeof SETTINGS_GUIDE_STEPS)[number];

export type SettingsGuideAccountMode =
	| "signed_out"
	| "signed_in_community"
	| "pro"
	| "enterprise";

export interface SettingsGuideAccountViewModel {
	mode: SettingsGuideAccountMode;
	isSignedIn: boolean;
	hasPaidAccess: boolean;
	title: string;
	statusLabel: string;
	description: string;
	detail: string;
	proSyncLine: string;
}

export interface SettingsGuideGroqStepViewModel {
	title: string;
	description: string;
	helper: string | null;
	submitLabel: string;
}

export interface SettingsGuideWrapupViewModel {
	title: string;
	description: string;
	detail: string;
}

function accountEmailLabel(state: LicenseState): string {
	return state.email?.trim() || "this account";
}

export function buildSettingsGuideAccountViewModel(
	state: LicenseState | null | undefined,
): SettingsGuideAccountViewModel {
	const proSyncLine =
		"Settings sync is Pro-only. You can sign in for free now, then upgrade later for sync and managed inference.";

	if (!state || state.status === "signed_out") {
		return {
			mode: "signed_out",
			isSignedIn: false,
			hasPaidAccess: false,
			title: "How would you like to continue?",
			statusLabel: "Not signed in",
			description:
				"An account is the path to settings sync and managed models. Local and bring-your-own-key features work without one.",
			detail:
				"Continue without an account to finish local/BYOK setup, or sign in now so your account is ready for Pro later.",
			proSyncLine,
		};
	}

	if (state.tier === "personal") {
		return {
			mode: "pro",
			isSignedIn: true,
			hasPaidAccess: true,
			title: "You’re signed in with Pro",
			statusLabel: "Pro active",
			description:
				"This account has paid Personal/Pro access. Pro-only features can use settings sync and managed inference once their launch paths are enabled.",
			detail: `Signed in as ${accountEmailLabel(state)}.`,
			proSyncLine,
		};
	}

	if (state.tier === "enterprise") {
		return {
			mode: "enterprise",
			isSignedIn: true,
			hasPaidAccess: true,
			title: "You’re signed in with managed business access",
			statusLabel: "Managed Business active",
			description:
				"This account is connected to an organization. Enterprise setup remains a later lane, but this session is already authenticated.",
			detail: `Signed in as ${accountEmailLabel(state)}.`,
			proSyncLine,
		};
	}

	return {
		mode: "signed_in_community",
		isSignedIn: true,
		hasPaidAccess: false,
		title: "You’re signed in — still Community/BYOK",
		statusLabel: "Signed-in Community",
		description:
			"No subscription is attached, so Kolboo keeps behaving like the free Community/BYOK app.",
		detail: `Signed in as ${accountEmailLabel(state)}. Payment is optional; upgrade later when you want Pro features.`,
		proSyncLine,
	};
}

export function buildSettingsGuideGroqStepViewModel(
	account: SettingsGuideAccountViewModel,
): SettingsGuideGroqStepViewModel {
	if (account.hasPaidAccess) {
		return {
			title: "Optional BYOK provider setup",
			description:
				"Your account is already signed in with paid access. Managed launch paths can use that where enabled, but you can still add a Groq key now if you want a BYOK fallback.",
			helper:
				"Skipping this step is fine. You can keep using your managed account path where available and add API keys later in Settings.",
			submitLabel: "Save key",
		};
	}

	return {
		title: "Create a Groq API key",
		description:
			"Groq provides free voice dictation (Whisper) and fast LLM rewriting. Create an API key here:",
		helper:
			"If you want to stay purely local for now, you can skip this step and come back later from Settings.",
		submitLabel: "Set key",
	};
}

export function buildSettingsGuideWrapupViewModel(
	account: SettingsGuideAccountViewModel,
): SettingsGuideWrapupViewModel {
	switch (account.mode) {
		case "signed_in_community":
			return {
				title: "You’re signed in and ready",
				description:
					"You finished setup in signed-in Community/BYOK mode. No subscription is attached yet, so Kolboo stays free and local/BYOK until you upgrade.",
				detail: account.proSyncLine,
			};
		case "pro":
			return {
				title: "You’re good to go with Pro",
				description:
					"You finished setup with paid Personal/Pro access. Managed and sync features can light up where their launch paths are enabled, and BYOK providers stay available too.",
				detail: account.detail,
			};
		case "enterprise":
			return {
				title: "You’re signed in with managed business access",
				description:
					"You finished setup with an authenticated organization-backed session. Enterprise admin flows remain a later lane, but this desktop session is already signed in.",
				detail: account.detail,
			};
		default:
			return {
				title: "You’re good to go",
				description:
					"You finished setup in Community/BYOK mode without signing in. You can keep using local/BYOK providers now and add an account later from Settings.",
				detail: account.proSyncLine,
			};
	}
}
