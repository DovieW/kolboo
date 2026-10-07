import type { LicenseState } from "../../lib/tauri";
import { isCloudServiceAvailable } from "../../lib/cloudService";

// Keep the first-run guide order centralized here so the setup copy/tests can
// assert that account setup stays ahead of provider configuration.
export const SETTINGS_GUIDE_STEPS = [
	"account",
	"groq",
	"dictation",
	"wrapup",
] as const;

export type SettingsGuideStep = (typeof SETTINGS_GUIDE_STEPS)[number];

export function buildSettingsGuideSteps(
	hasManagedAccess: boolean,
): [SettingsGuideStep, ...SettingsGuideStep[]] {
	if (!isCloudServiceAvailable()) return ["groq", "dictation", "wrapup"];
	return hasManagedAccess
		? ["account", "dictation", "wrapup"]
		: [...SETTINGS_GUIDE_STEPS];
}

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
		"Approved beta accounts include managed models and settings sync. Your own keys work without an account.";

	if (!isCloudServiceAvailable() || !state || state.status === "signed_out") {
		return {
			mode: "signed_out",
			isSignedIn: false,
			hasPaidAccess: false,
			title: "Account setup",
			statusLabel: "Not signed in",
			description:
				"An account is the path to settings sync and managed models. Local and bring-your-own-key features work without one.",
			detail:
				"Continue without an account to finish local/BYOK setup, or sign in now so your account is ready for Pro later.",
			proSyncLine,
		};
	}

	if (state.tier === "personal" && (state.status === "active" || state.status === "grace")) {
		return {
			mode: "pro",
			isSignedIn: true,
			hasPaidAccess: true,
			title: "You’re signed in with Pro",
			statusLabel: "Pro active",
			description:
				"Your Pro account includes settings sync and managed models.",
			detail: `Signed in as ${accountEmailLabel(state)}.`,
			proSyncLine,
		};
	}

	if (state.tier === "enterprise" && (state.status === "active" || state.status === "grace")) {
		return {
			mode: "enterprise",
			isSignedIn: true,
			hasPaidAccess: true,
			title: "You’re signed in with managed business access",
			statusLabel: "Managed Business active",
			description:
				"Your Managed Business account includes organization-managed settings and models.",
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
			"Use your own provider keys, or request beta access.",
		detail: `Signed in as ${accountEmailLabel(state)}.`,
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
				"You can add a Groq key as an optional BYOK fallback from Settings.",
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
			"You can add another provider in Settings.",
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
					"Your own provider keys remain available. Pro beta access requires an approved email.",
				detail: account.proSyncLine,
			};
		case "pro":
			return {
				title: "You’re good to go with Pro",
				description:
					"Your Pro account includes settings sync and managed models. BYOK providers remain available in Settings.",
				detail: account.detail,
			};
		case "enterprise":
			return {
				title: "You’re signed in with managed business access",
				description:
					"Your Managed Business account includes organization-managed settings and models.",
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
