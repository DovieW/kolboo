import { formatEnterprisePersonaLabel } from "../../lib/queries/enterprisePersona";
import type { EnterprisePersonaState } from "../../lib/tauri";

export interface TestPersonaIndicatorModel {
	contextLabel: string;
	personaLabel: string;
	environmentLabel: string;
	showTestAccessBadge: boolean;
	testAccessLabel: string | null;
}

export function buildTestPersonaIndicatorModel(
	state: EnterprisePersonaState | null | undefined,
): TestPersonaIndicatorModel | null {
	if (!state) return null;
	const hasPersonaContext = Boolean(
		state.context_key || state.persona_type || state.test_access_active,
	);
	if (!hasPersonaContext) return null;

	const contextLabel = state.context_key?.trim() || "None";
	const personaLabel = formatEnterprisePersonaLabel(state.persona_type);
	const environmentLabel = state.environment.toUpperCase();
	const testAccessLabel = state.test_access_expires_at
		? `until ${new Date(state.test_access_expires_at).toLocaleString()}`
		: null;

	return {
		contextLabel,
		personaLabel,
		environmentLabel,
		showTestAccessBadge: state.test_access_active,
		testAccessLabel,
	};
}
