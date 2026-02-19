import { describe, expect, it } from "vitest";
import type { EnterprisePersonaState, SettingsChangedPayload } from "../tauri";
import {
	applyPersonaEventPayload,
	formatEnterprisePersonaLabel,
} from "./enterprisePersona";

describe("enterprisePersona query helpers", () => {
	it("maps persona labels deterministically", () => {
		expect(formatEnterprisePersonaLabel("byok")).toBe("BYOK");
		expect(formatEnterprisePersonaLabel("managed")).toBe("Managed");
		expect(formatEnterprisePersonaLabel("mixed-policy")).toBe("Mixed policy");
		expect(formatEnterprisePersonaLabel(null)).toBe("Not set");
	});

	it("applies settings-changed payload patch to persona state", () => {
		const base: EnterprisePersonaState = {
			context_key: null,
			persona_type: null,
			test_access_active: false,
			test_access_expires_at: null,
			environment: "local",
			source: "none",
			updated_at: null,
		};

		const payload: SettingsChangedPayload = {
			persona_context_key: "pr-1024",
			persona_type: "managed",
			test_access_active: true,
		};

		const next = applyPersonaEventPayload(base, payload);
		expect(next.context_key).toBe("pr-1024");
		expect(next.persona_type).toBe("managed");
		expect(next.test_access_active).toBe(true);
		expect(next.environment).toBe("preview");
		expect(next.source).toBe("event");
	});
});
