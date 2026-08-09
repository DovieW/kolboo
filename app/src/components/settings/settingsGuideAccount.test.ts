import { describe, expect, it } from "vitest";
import type { LicenseState } from "../../lib/tauri";
import {
	buildSettingsGuideAccountViewModel,
	buildSettingsGuideGroqStepViewModel,
	buildSettingsGuideSteps,
	buildSettingsGuideWrapupViewModel,
	SETTINGS_GUIDE_STEPS,
} from "./settingsGuideAccount";

const baseState: LicenseState = {
	tier: "community",
	status: "active",
	user_id: "user-123",
	email: "dovie@example.test",
	org: null,
	expires_at: null,
	cached_at: "2026-05-20T12:00:00.000Z",
	last_validated_at: "2026-05-20T12:00:00.000Z",
	usage: {
		stt_seconds_used: 0,
		llm_tokens_used: 0,
		requests_today: 0,
	},
	limits: {
		stt_seconds_monthly: 0,
		llm_tokens_monthly: 0,
		requests_per_day: 0,
	},
	portal_available: false,
};

describe("buildSettingsGuideAccountViewModel", () => {
	it("keeps the account step ahead of provider setup", () => {
		expect(SETTINGS_GUIDE_STEPS).toEqual([
			"account",
			"groq",
			"dictation",
			"wrapup",
		]);
	});

	it("skips BYOK provider onboarding for signed-in users", () => {
		expect(buildSettingsGuideSteps(true)).toEqual([
			"account",
			"dictation",
			"wrapup",
		]);
		expect(buildSettingsGuideSteps(false)).toEqual(SETTINGS_GUIDE_STEPS);
	});

	it("does not require an account for Community/BYOK setup", () => {
		const model = buildSettingsGuideAccountViewModel(null);

		expect(model.mode).toBe("signed_out");
		expect(model.isSignedIn).toBe(false);
		expect(model.title).toBe("Account setup");
		expect(model.description).toContain("Local and bring-your-own-key");
		expect(model.description).toContain("without one");
	});

	it("treats signed-in unpaid users as Community/BYOK", () => {
		const model = buildSettingsGuideAccountViewModel(baseState);

		expect(model.mode).toBe("signed_in_community");
		expect(model.isSignedIn).toBe(true);
		expect(model.hasPaidAccess).toBe(false);
		expect(model.statusLabel).toBe("Signed-in Community");
		expect(model.detail).toContain("Payment is optional");
	});

	it("keeps settings sync scoped to Pro", () => {
		const model = buildSettingsGuideAccountViewModel({
			...baseState,
			tier: "personal",
		});

		expect(model.mode).toBe("pro");
		expect(model.hasPaidAccess).toBe(true);
		expect(model.proSyncLine).toContain("Settings sync is Pro-only");
		expect(model.description).toContain("settings sync");
	});

	it("keeps the Groq step optional for paid accounts", () => {
		const account = buildSettingsGuideAccountViewModel({
			...baseState,
			tier: "personal",
		});
		const groq = buildSettingsGuideGroqStepViewModel(account);

		expect(groq.title).toContain("Optional BYOK");
		expect(groq.description).toContain("BYOK fallback");
		expect(groq.helper).toContain("Skipping this step is fine");
		expect(groq.submitLabel).toBe("Save key");
	});

	it("summarizes signed-in Community mode during wrap-up", () => {
		const account = buildSettingsGuideAccountViewModel(baseState);
		const wrapup = buildSettingsGuideWrapupViewModel(account);

		expect(wrapup.title).toContain("signed in");
		expect(wrapup.description).toContain("signed-in Community/BYOK mode");
		expect(wrapup.detail).toContain("Settings sync is Pro-only");
	});
});
