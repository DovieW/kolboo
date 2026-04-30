import { describe, expect, it } from "vitest";
import {
	evaluateTokenExchangeDecision,
	getActiveTokenExchangeTriggers,
} from "./tokenExchangeGate";

describe("tokenExchangeGate", () => {
	it("keeps direct IdP tokens when no triggers are active", () => {
		expect(
			evaluateTokenExchangeDecision({
				multi_idp_required: false,
				kill_switch_required: false,
				embedded_claims_required: false,
				desktop_idp_agnostic_required: false,
			}),
		).toBe("direct_idp_token");
	});

	it("adopts token exchange when any trigger is active", () => {
		expect(
			evaluateTokenExchangeDecision({
				multi_idp_required: false,
				kill_switch_required: true,
				embedded_claims_required: false,
				desktop_idp_agnostic_required: false,
			}),
		).toBe("adopt_token_exchange");
	});

	it("lists only the active trigger fields", () => {
		expect(
			getActiveTokenExchangeTriggers({
				multi_idp_required: true,
				kill_switch_required: false,
				embedded_claims_required: true,
				desktop_idp_agnostic_required: false,
			}),
		).toEqual(["multi_idp_required", "embedded_claims_required"]);
	});
});
