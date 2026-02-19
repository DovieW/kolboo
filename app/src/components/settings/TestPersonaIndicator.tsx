import { Badge, Group, Text } from "@mantine/core";
import {
	formatEnterprisePersonaLabel,
	useEnterprisePersonaState,
} from "../../lib/queries/enterprisePersona";
import type { EnterprisePersonaState } from "../../lib/tauri";
import { SettingsRow } from "./SettingsRow";

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

export function TestPersonaIndicator() {
	const personaState = useEnterprisePersonaState();
	const model = buildTestPersonaIndicatorModel(personaState.data);

	if (!model) return null;

	return (
		<SettingsRow
			label="Non-production persona"
			description="Shows active deterministic test persona context for local/preview/staging validation."
			right={
				<Group gap={6} justify="flex-end" align="center" wrap="wrap">
					<Badge variant="light" color="orange">
						{model.environmentLabel}
					</Badge>
					<Badge variant="outline" color="gray">
						{model.personaLabel}
					</Badge>
					{model.showTestAccessBadge ? (
						<Badge color="teal">Test access active</Badge>
					) : null}
					<Text size="xs" c="dimmed">
						Context: {model.contextLabel}
					</Text>
					{model.testAccessLabel ? (
						<Text size="xs" c="dimmed">
							{model.testAccessLabel}
						</Text>
					) : null}
				</Group>
			}
		/>
	);
}
