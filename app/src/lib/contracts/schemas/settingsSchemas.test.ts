import fs from "node:fs";
import { describe, expect, it } from "vitest";
import type {
	HotkeyConfig,
	IntentRouterSettings,
	ProxySettings,
	RewritePreset,
	RewriteProgramPromptProfile,
	TrustedCaCertificate,
} from "../../tauri";

function readSchema(schemaFile: string): {
	properties?: Record<string, unknown>;
	definitions?: Record<string, { properties?: Record<string, unknown> }>;
} {
	const schemaPath = new URL(
		`../../../../src-tauri/gen/schemas/${schemaFile}`,
		import.meta.url,
	);
	const rawSchema = fs.readFileSync(schemaPath, "utf8").replace(/^\uFEFF/, "");
	return JSON.parse(rawSchema) as {
		properties?: Record<string, unknown>;
		definitions?: Record<string, { properties?: Record<string, unknown> }>;
	};
}

describe("schema contract: settings shapes", () => {
	it("keeps ProxySettings shape aligned with backend JSON schema", () => {
		const proxySettings: ProxySettings = {
			mode: "system",
			manual: {
				proxy_url: "",
				no_proxy: "",
				username: "",
				password: "",
			},
			trusted_ca_certificates: [],
			danger_accept_invalid_certs: false,
		};

		const schema = readSchema("proxy-settings.schema.json");
		const schemaProps = schema.properties ?? {};
		const missingProxyKeys = Object.keys(proxySettings).filter(
			(k) => !(k in schemaProps),
		);

		const manualProps =
			schema.definitions?.ManualProxySettings?.properties ?? {};
		const missingManualKeys = Object.keys(proxySettings.manual).filter(
			(k) => !(k in manualProps),
		);

		const sampleCert: TrustedCaCertificate = {
			id: "",
			file_name: "",
			format: "pem",
			data_base64: "",
		};
		const certProps =
			schema.definitions?.TrustedCaCertificate?.properties ?? {};
		const missingCertKeys = Object.keys(sampleCert).filter(
			(k) => !(k in certProps),
		);

		expect(
			missingProxyKeys,
			`ProxySettings keys missing in backend schema: ${missingProxyKeys.join(
				", ",
			)}`,
		).toEqual([]);

		expect(
			missingManualKeys,
			`ManualProxySettings keys missing in backend schema: ${missingManualKeys.join(
				", ",
			)}`,
		).toEqual([]);

		expect(
			missingCertKeys,
			`TrustedCaCertificate keys missing in backend schema: ${missingCertKeys.join(
				", ",
			)}`,
		).toEqual([]);
	});

	it("keeps HotkeyConfig shape aligned with backend JSON schema", () => {
		const hotkey: HotkeyConfig = {
			modifiers: [],
			key: "F1",
		};

		const schema = readSchema("hotkey-config.schema.json");
		const schemaProps = schema.properties ?? {};
		const missingHotkeyKeys = Object.keys(hotkey).filter(
			(k) => !(k in schemaProps),
		);

		expect(
			missingHotkeyKeys,
			`HotkeyConfig keys missing in backend schema: ${missingHotkeyKeys.join(
				", ",
			)}`,
		).toEqual([]);
	});

	it("keeps IntentRouterSettings shape aligned with backend JSON schema", () => {
		const sampleRouter: IntentRouterSettings = {
			enabled: true,
			strategy: "off",
			embedding_provider: null,
			embedding_model: null,
			pick_highest_score: null,
			similarity_threshold: null,
			similarity_margin: null,
			llm_provider: null,
			llm_model: null,
			openai_reasoning_effort: null,
			gemini_thinking_budget: null,
			gemini_thinking_level: null,
			anthropic_thinking_budget: null,
			llm_system_prompt: null,
		};

		const schema = readSchema("intent-router-settings.schema.json");
		const schemaProps = schema.properties ?? {};
		const missingKeys = Object.keys(sampleRouter).filter(
			(k) => !(k in schemaProps),
		);

		expect(
			missingKeys,
			`IntentRouterSettings keys missing in backend schema: ${missingKeys.join(
				", ",
			)}`,
		).toEqual([]);
	});

	it("keeps RewritePreset shape aligned with backend JSON schema", () => {
		const samplePreset: RewritePreset = {
			id: "",
			name: "",
			description: null,
			routing_hints: null,
			cleanup_prompt_sections: null,
			rewrite_llm_enabled: true,
			stt_provider: null,
			stt_model: null,
			stt_timeout_seconds: null,
			llm_provider: null,
			llm_model: null,
			openai_reasoning_effort: null,
			gemini_thinking_budget: null,
			gemini_thinking_level: null,
			anthropic_thinking_budget: null,
			sound_enabled: null,
			playing_audio_handling: null,
			overlay_mode: null,
			widget_position: null,
			output_mode: null,
			output_hit_enter: null,
		};

		const schema = readSchema("rewrite-preset.schema.json");
		const schemaProps = schema.properties ?? {};
		const missingKeys = Object.keys(samplePreset).filter(
			(k) => !(k in schemaProps),
		);

		expect(
			missingKeys,
			`RewritePreset keys missing in backend schema: ${missingKeys.join(", ")}`,
		).toEqual([]);
	});

	it("keeps RewriteProgramPromptProfile shape aligned with backend JSON schema", () => {
		const sampleProfile: RewriteProgramPromptProfile = {
			id: "default",
			name: "Default",
			program_paths: [],
			cleanup_prompt_sections: null,
			presets: null,
			default_preset_id: null,
			default_preset_description: null,
			default_target_rewrite_llm_enabled: true,
			active_preset_id: null,
			router: null,
			rewrite_llm_enabled: null,
			stt_provider: null,
			stt_model: null,
			stt_timeout_seconds: null,
			llm_provider: null,
			llm_model: null,
			openai_reasoning_effort: null,
			gemini_thinking_budget: null,
			gemini_thinking_level: null,
			anthropic_thinking_budget: null,
			quick_ask_provider: null,
			quick_ask_model: null,
			quick_ask_system_prompt: null,
			context_grab_method: null,
			rewrite_include_clipboard_context: null,
			quick_replace_include_clipboard_context: null,
			quick_ask_include_clipboard_context: null,
			quick_replace_enabled: null,
			quick_replace_provider: null,
			quick_replace_model: null,
			quick_replace_system_prompt: null,
			quick_ask_openai_reasoning_effort: null,
			quick_ask_gemini_thinking_budget: null,
			quick_ask_gemini_thinking_level: null,
			quick_ask_anthropic_thinking_budget: null,
			sound_enabled: null,
			playing_audio_handling: null,
			overlay_mode: null,
			widget_position: null,
			output_mode: null,
			output_hit_enter: null,
		};

		const schema = readSchema("rewrite-program-profile.schema.json");
		const schemaProps = schema.properties ?? {};
		const missingKeys = Object.keys(sampleProfile).filter(
			(k) => !(k in schemaProps),
		);

		expect(
			missingKeys,
			`RewriteProgramPromptProfile keys missing in backend schema: ${missingKeys.join(
				", ",
			)}`,
		).toEqual([]);
	});
});
