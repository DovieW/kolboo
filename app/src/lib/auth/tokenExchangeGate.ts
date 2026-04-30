import type {
	TokenExchangeDecision,
	TokenExchangeTriggerSet,
} from "../tauri/types";

export const TOKEN_EXCHANGE_TRIGGER_FIELDS = [
	"multi_idp_required",
	"kill_switch_required",
	"embedded_claims_required",
	"desktop_idp_agnostic_required",
] as const;

export type TokenExchangeTriggerField =
	(typeof TOKEN_EXCHANGE_TRIGGER_FIELDS)[number];

export type TokenExchangeDecisionInput = Pick<
	TokenExchangeTriggerSet,
	TokenExchangeTriggerField
>;

export function getActiveTokenExchangeTriggers(
	input: TokenExchangeDecisionInput,
): TokenExchangeTriggerField[] {
	return TOKEN_EXCHANGE_TRIGGER_FIELDS.filter((field) => Boolean(input[field]));
}

export function evaluateTokenExchangeDecision(
	input: TokenExchangeDecisionInput,
): TokenExchangeDecision {
	return getActiveTokenExchangeTriggers(input).length > 0
		? "adopt_token_exchange"
		: "direct_idp_token";
}
