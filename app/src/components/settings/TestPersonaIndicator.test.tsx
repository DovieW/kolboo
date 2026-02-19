import { describe, expect, it } from "vitest";
import type { EnterprisePersonaState } from "../../lib/tauri";
import { buildTestPersonaIndicatorModel } from "./TestPersonaIndicator";

describe("TestPersonaIndicator model", () => {
	it("returns null when no persona context is active", () => {
		const state: EnterprisePersonaState = {
			context_key: null,
			persona_type: null,
			test_access_active: false,
			test_access_expires_at: null,
			environment: "local",
			source: "none",
			updated_at: null,
		};

		expect(buildTestPersonaIndicatorModel(state)).toBeNull();
	});

	it("builds a model for active non-production persona context", () => {
		const state: EnterprisePersonaState = {
			context_key: "pr-777",
			persona_type: "mixed-policy",
			test_access_active: true,
			test_access_expires_at: "2026-02-19T12:00:00.000Z",
			environment: "preview",
			source: "event",
			updated_at: "2026-02-19T10:00:00.000Z",
		};

		const model = buildTestPersonaIndicatorModel(state);
		expect(model).not.toBeNull();
		expect(model?.environmentLabel).toBe("PREVIEW");
		expect(model?.contextLabel).toBe("pr-777");
		expect(model?.personaLabel).toBe("Mixed policy");
		expect(model?.showTestAccessBadge).toBe(true);
	});
});
