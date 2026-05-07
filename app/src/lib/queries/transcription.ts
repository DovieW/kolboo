import { useMutation, useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";

import {
	type IterateRewritePromptResponse,
	llmAPI,
	sttAPI,
	type TestLlmRewriteResponse,
	type TestRewriteWithPromptResponse,
} from "../tauri";
import {
	createDefaultSectionsQueryFn,
	createHasLastAudioQueryFn,
} from "./queryFns";
import { queryFnDeps } from "./shared";

// These hooks support rewriting and last-audio transcription experiments, so
// they share one module instead of hiding that workflow across unrelated files.
export function useTypeText() {
	return useMutation({
		mutationFn: (text: string) => invoke("type_text", { text }),
	});
}

export function useTestLlmRewrite() {
	return useMutation({
		mutationFn: (params: {
			transcript: string;
			profileId?: string | null;
		}): Promise<TestLlmRewriteResponse> => llmAPI.testLlmRewrite(params),
	});
}

export function useIterateRewritePrompt() {
	return useMutation({
		mutationFn: (params: {
			transcript: string;
			problemOutput: string;
			desiredOutput?: string | null;
			currentPrompt: string;
			profileId?: string | null;
			mode?: "fixed" | "new";

			llmProvider?: string | null;
			llmModel?: string | null;
			openAiReasoningEffort?: "none" | "low" | "medium" | "high" | null;
			geminiThinkingLevel?: "minimal" | "low" | "medium" | "high" | null;
			geminiThinkingBudget?: number | null;
			anthropicThinkingBudget?: number | null;
		}): Promise<IterateRewritePromptResponse> =>
			llmAPI.iterateRewritePrompt(params),
	});
}

export function useTestRewriteWithPrompt() {
	return useMutation({
		mutationFn: (params: {
			transcript: string;
			prompt: string;
			profileId?: string | null;
		}): Promise<TestRewriteWithPromptResponse> =>
			llmAPI.testRewriteWithPrompt(params),
	});
}

export function useTestSttTranscribeLastAudio() {
	return useMutation({
		mutationFn: (params: { profileId?: string | null }): Promise<string> =>
			sttAPI.testTranscribeLastAudio(params),
	});
}

export function useHasLastAudioForSttTest() {
	return useQuery({
		queryKey: ["sttLastAudioAvailable"],
		queryFn: createHasLastAudioQueryFn(queryFnDeps),
		staleTime: 0,
		refetchOnWindowFocus: true,
		// Very cheap boolean check; polling keeps the UI in sync when the user
		// records audio via hotkey while the settings page is open.
		refetchInterval: 2000,
	});
}

export function useDefaultSections() {
	return useQuery({
		queryKey: ["defaultSections"],
		queryFn: createDefaultSectionsQueryFn(queryFnDeps),
		staleTime: Number.POSITIVE_INFINITY,
	});
}
