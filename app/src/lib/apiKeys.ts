export interface ApiKeyConfig {
	id: string;
	label: string;
	placeholder: string;
	storeKey: string;
	getKeyUrl: string;
}

export type ApiKeyMutationIntent =
  | {
      kind: "save";
      value: string;
    }
  | {
      kind: "clear";
    };

export function resolveApiKeyMutationIntent(params: {
  draftValue: string;
  savedValue: string | null | undefined;
}): ApiKeyMutationIntent | null {
  const trimmedDraft = params.draftValue.trim();
  const trimmedSaved = (params.savedValue ?? "").trim();

  if (trimmedDraft.length === 0) {
    return trimmedSaved.length > 0 ? { kind: "clear" } : null;
  }

  if (trimmedDraft === trimmedSaved) {
    return null;
  }

  return {
    kind: "save",
    value: trimmedDraft,
  };
}

export const API_KEYS: ApiKeyConfig[] = [
	{
		id: "groq",
		label: "Groq",
		placeholder: "Enter API key",
		storeKey: "groq_api_key",
		getKeyUrl: "https://console.groq.com/keys",
	},
	{
		id: "cerebras",
		label: "Cerebras",
		placeholder: "Enter API key",
		storeKey: "cerebras_api_key",
		getKeyUrl: "https://cloud.cerebras.ai/platform",
	},
	{
		id: "cohere",
		label: "Cohere",
		placeholder: "Enter API key",
		storeKey: "cohere_api_key",
		getKeyUrl: "https://dashboard.cohere.com/api-keys",
	},
	{
		id: "assemblyai",
		label: "AssemblyAI",
		placeholder: "Enter API key",
		storeKey: "assemblyai_api_key",
		getKeyUrl: "https://www.assemblyai.com/dashboard/api-keys",
	},
	{
		id: "speechmatics",
		label: "Speechmatics",
		placeholder: "Enter API key",
		storeKey: "speechmatics_api_key",
		getKeyUrl: "https://portal.speechmatics.com/settings/api-keys",
	},
	{
		id: "elevenlabs",
		label: "ElevenLabs",
		placeholder: "Enter API key",
		storeKey: "elevenlabs_api_key",
		getKeyUrl: "https://elevenlabs.io/app/settings/api-keys",
	},
	{
		id: "aquavoice",
		label: "Aquavoice",
		placeholder: "Enter API key",
		storeKey: "aquavoice_api_key",
		getKeyUrl: "https://app.aquavoice.com/api-dashboard?tab=keys",
	},
	{
		id: "gemini",
		label: "Google AI Studio",
		placeholder: "Enter API key",
		storeKey: "gemini_api_key",
		getKeyUrl: "https://aistudio.google.com/apikey",
	},
	{
		id: "openai",
		label: "OpenAI",
		placeholder: "Enter API key",
		storeKey: "openai_api_key",
		getKeyUrl: "https://platform.openai.com/api-keys",
	},
	{
		id: "fireworks",
		label: "Fireworks",
		placeholder: "Enter API key",
		storeKey: "fireworks_api_key",
		getKeyUrl: "https://app.fireworks.ai/settings/users/api-keys",
	},
	{
		id: "deepgram",
		label: "Deepgram",
		placeholder: "Enter API key",
		storeKey: "deepgram_api_key",
		getKeyUrl: "https://console.deepgram.com/project",
	},
	{
		id: "anthropic",
		label: "Anthropic",
		placeholder: "Enter API key",
		storeKey: "anthropic_api_key",
		getKeyUrl: "https://platform.claude.com/settings/keys",
	},
];

export const API_KEY_STORE_KEYS = API_KEYS.map((k) => k.storeKey);
