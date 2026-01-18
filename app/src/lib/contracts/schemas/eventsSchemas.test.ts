import fs from "node:fs";
import { describe, expect, it } from "vitest";
import type {
	ConnectionStateChangedPayload,
	EmptyEventPayload,
	MicTestAudioLevelPayload,
	OverlayAudioLevelPayload,
	PipelineErrorPayload,
	PipelineStateEvent,
	PipelineTranscriptReadyPayload,
	QuickAskAnswerPayload,
	QuickAskStartedPayload,
	SettingsChangedPayload,
	SystemEvent,
} from "../../tauri";
import { resolveSchemasDir, schemaPath } from "../contractTestPaths";

type SchemaDefinition = {
	properties?: Record<string, unknown>;
	enum?: string[];
	oneOf?: Array<{ enum?: string[] }>;
};

type SchemaVariant = {
	properties?: Record<string, unknown>;
	enum?: string[];
};

function readSchema(schemaFile: string): {
	properties?: Record<string, unknown>;
	definitions?: Record<string, SchemaDefinition>;
	oneOf?: SchemaVariant[];
	anyOf?: SchemaVariant[];
	enum?: string[];
	type?: string;
} {
	const resolvedPath = schemaPath(schemaFile);
	if (!fs.existsSync(resolvedPath)) {
		throw new Error(`Schema missing: ${schemaFile}`);
	}
	const rawSchema = fs
		.readFileSync(resolvedPath, "utf8")
		.replace(/^\uFEFF/, "");
	return JSON.parse(rawSchema) as {
		properties?: Record<string, unknown>;
		definitions?: Record<string, SchemaDefinition>;
		oneOf?: SchemaVariant[];
		anyOf?: SchemaVariant[];
		enum?: string[];
		type?: string;
	};
}

function hasSchemas(): boolean {
	const schemasDir = resolveSchemasDir();
	return fs.existsSync(schemasDir) && fs.readdirSync(schemasDir).length > 0;
}

function assertNullEventSchema(schemaFile: string, label: string) {
	const sample: EmptyEventPayload = null;
	const schema = readSchema(schemaFile);

	expect(sample).toBeNull();
	expect(schema.type, `${label} schema should be null`).toBe("null");
}

describe.skipIf(!hasSchemas())("schema contract: event payloads", () => {
	it("keeps SystemEvent shape aligned with backend JSON schema", () => {
		const sample: SystemEvent = {
			timestamp: new Date().toISOString(),
			event_type: "debug",
			message: "hello",
			details: null,
		};

		const schema = readSchema("system-event.schema.json");
		const schemaProps = schema.properties ?? {};
		const missingKeys = Object.keys(sample).filter((k) => !(k in schemaProps));

		expect(
			missingKeys,
			`SystemEvent keys missing in backend schema: ${missingKeys.join(", ")}`,
		).toEqual([]);
	});

	it("keeps PipelineErrorPayload shape aligned with backend JSON schema", () => {
		const sample: PipelineErrorPayload = {
			message: "boom",
			request_id: null,
		};

		const schema = readSchema("pipeline-error-payload.schema.json");
		const schemaProps = schema.properties ?? {};
		const missingKeys = Object.keys(sample).filter((k) => !(k in schemaProps));

		expect(
			missingKeys,
			`PipelineErrorPayload keys missing in backend schema: ${missingKeys.join(
				", ",
			)}`,
		).toEqual([]);
	});

	it("keeps PipelineStateEvent values aligned with backend JSON schema", () => {
		const sampleState: PipelineStateEvent = "idle";

		const schema = readSchema("pipeline-state-changed.schema.json");
		const enumValues =
			schema.enum ?? schema.oneOf?.flatMap((v) => v.enum ?? []) ?? [];
		expect(enumValues).toContain(sampleState);
		expect(enumValues).toContain("recording");
		expect(enumValues).toContain("transcribing");
		expect(enumValues).toContain("routing");
		expect(enumValues).toContain("rewriting");
		expect(enumValues).toContain("error");
	});

	it("keeps PipelineTranscriptReadyPayload aligned with backend JSON schema", () => {
		const sample: PipelineTranscriptReadyPayload = "hello";

		const schema = readSchema("pipeline-transcript-ready.schema.json");

		expect(typeof sample).toBe("string");
		expect(schema.type).toBe("string");
	});

	it("keeps pipeline-recording-started payload aligned with backend JSON schema", () => {
		assertNullEventSchema(
			"pipeline-recording-started.schema.json",
			"pipeline-recording-started",
		);
	});

	it("keeps pipeline-transcription-started payload aligned with backend JSON schema", () => {
		assertNullEventSchema(
			"pipeline-transcription-started.schema.json",
			"pipeline-transcription-started",
		);
	});

	it("keeps pipeline-routing-started payload aligned with backend JSON schema", () => {
		assertNullEventSchema(
			"pipeline-routing-started.schema.json",
			"pipeline-routing-started",
		);
	});

	it("keeps pipeline-rewriting-started payload aligned with backend JSON schema", () => {
		assertNullEventSchema(
			"pipeline-rewriting-started.schema.json",
			"pipeline-rewriting-started",
		);
	});

	it("keeps pipeline-cancelled payload aligned with backend JSON schema", () => {
		assertNullEventSchema(
			"pipeline-cancelled.schema.json",
			"pipeline-cancelled",
		);
	});

	it("keeps pipeline-reset payload aligned with backend JSON schema", () => {
		assertNullEventSchema("pipeline-reset.schema.json", "pipeline-reset");
	});

	it("keeps recording-start payload aligned with backend JSON schema", () => {
		assertNullEventSchema("recording-start.schema.json", "recording-start");
	});

	it("keeps recording-stop payload aligned with backend JSON schema", () => {
		assertNullEventSchema("recording-stop.schema.json", "recording-stop");
	});

	it("keeps overlay-hide-requested payload aligned with backend JSON schema", () => {
		assertNullEventSchema(
			"overlay-hide-requested.schema.json",
			"overlay-hide-requested",
		);
	});

	it("keeps history-changed payload aligned with backend JSON schema", () => {
		assertNullEventSchema("history-changed.schema.json", "history-changed");
	});

	it("keeps stats-changed payload aligned with backend JSON schema", () => {
		assertNullEventSchema("stats-changed.schema.json", "stats-changed");
	});

	it("keeps settings-changed payload aligned with backend JSON schema", () => {
		const sample: SettingsChangedPayload = {};

		const schema = readSchema("settings-changed.schema.json");

		expect(sample).toEqual({});
		expect(schema.type).toBe("object");
	});

	it("keeps connection-state-changed payload aligned with backend JSON schema", () => {
		const sample: ConnectionStateChangedPayload = { state: "idle" };

		const schema = readSchema("connection-state-changed.schema.json");
		const schemaProps = schema.properties ?? {};
		const missingKeys = Object.keys(sample).filter((k) => !(k in schemaProps));
		expect(
			missingKeys,
			`connection-state-changed keys missing in backend schema: ${missingKeys.join(
				", ",
			)}`,
		).toEqual([]);

		const enumValues =
			schema.definitions?.ConnectionStateEvent?.enum ??
			schema.definitions?.ConnectionStateEvent?.oneOf?.flatMap(
				(v) => v.enum ?? [],
			) ??
			[];
		expect(enumValues).toContain("idle");
		expect(enumValues).toContain("recording");
		expect(enumValues).toContain("processing");
		expect(enumValues).toContain("connecting");
		expect(enumValues).toContain("disconnected");
	});

	it("keeps OverlayAudioLevelPayload shape aligned with backend JSON schema", () => {
		const sample: OverlayAudioLevelPayload = {
			seq: 1,
			rms: 0,
			peak: 0,
			wave_seq: 1,
			mins: [],
			maxes: [],
		};

		const schema = readSchema("overlay-audio-level-payload.schema.json");
		const schemaProps = schema.properties ?? {};
		const missingKeys = Object.keys(sample).filter((k) => !(k in schemaProps));

		expect(
			missingKeys,
			`OverlayAudioLevelPayload keys missing in backend schema: ${missingKeys.join(
				", ",
			)}`,
		).toEqual([]);
	});

	it("keeps QuickAskStartedPayload shape aligned with backend JSON schema", () => {
		const sample: QuickAskStartedPayload = {
			question: "hello",
			provider: "openai",
			model: "gpt-4o",
		};

		const schema = readSchema("quick-ask-started-payload.schema.json");
		const schemaProps = schema.properties ?? {};
		const missingKeys = Object.keys(sample).filter((k) => !(k in schemaProps));

		expect(
			missingKeys,
			`QuickAskStartedPayload keys missing in backend schema: ${missingKeys.join(
				", ",
			)}`,
		).toEqual([]);
	});

	it("keeps QuickAskAnswerPayload shape aligned with backend JSON schema", () => {
		const sampleOk: QuickAskAnswerPayload = {
			ok: true,
			answer: "42",
			provider_used: "openai",
			model_used: "gpt-4o",
			duration_ms: 5,
		};

		const schema = readSchema("quick-ask-answer-payload.schema.json");
		const candidates = schema.oneOf ?? schema.anyOf ?? [];
		const okProps = candidates
			.map((c) => {
				const props = c.properties ?? {};
				if (Object.keys(props).length > 0) return props;
				const ref = (c as { $ref?: string }).$ref;
				if (!ref) return {};
				const refName = ref.split("/").pop();
				if (!refName) return {};
				return schema.definitions?.[refName]?.properties ?? {};
			})
			.find((props) => "answer" in props && "ok" in props);

		const missingOkKeys = Object.keys(sampleOk).filter(
			(k) => !(k in (okProps ?? {})),
		);

		expect(
			missingOkKeys,
			`QuickAskAnswerPayload (ok) keys missing in backend schema: ${missingOkKeys.join(
				", ",
			)}`,
		).toEqual([]);
	});

	it("keeps MicTestAudioLevelPayload shape aligned with backend JSON schema", () => {
		const sample: MicTestAudioLevelPayload = {
			active: true,
			session_id: 1,
			seq: 1,
			rms: 0,
			peak: 0,
		};

		const schema = readSchema("mic-test-audio-level-payload.schema.json");
		const schemaProps = schema.properties ?? {};
		const missingKeys = Object.keys(sample).filter((k) => !(k in schemaProps));

		expect(
			missingKeys,
			`MicTestAudioLevelPayload keys missing in backend schema: ${missingKeys.join(
				", ",
			)}`,
		).toEqual([]);
	});
});
