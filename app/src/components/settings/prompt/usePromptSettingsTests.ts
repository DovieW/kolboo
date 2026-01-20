import { type Dispatch, type SetStateAction, useRef, useState } from "react";

type Mutation<TArgs, TResult> = {
	mutate: (
		args: TArgs,
		options: {
			onSuccess: (res: TResult) => void;
			onError: (err: unknown) => void;
		},
	) => void;
};

type UsePromptSettingsTestsOptions = {
	activeProfileId: string;
	errorToMessage: (err: unknown) => string;
	testLlmRewrite: Mutation<
		{
			transcript: string;
			profileId: string;
		},
		{ output: string }
	>;
	testRewriteWithPrompt: Mutation<
		{
			transcript: string;
			prompt: string;
			profileId: string;
		},
		{ output: string }
	>;
	testSttLastAudio: Mutation<
		{
			profileId: string;
		},
		string
	>;
};

type PromptSettingsTestsState = {
	rewriteTestInput: string;
	setRewriteTestInput: Dispatch<SetStateAction<string>>;
	rewriteTestOutput: string;
	rewriteTestError: string;
	rewriteTestDurationMs: number | null;
	runRewriteTest: (promptOverride?: string) => void;
	sttTestOutput: string;
	sttTestError: string;
	sttTestDurationMs: number | null;
	handleRunSttTest: () => void;
};

export function usePromptSettingsTests({
	activeProfileId,
	errorToMessage,
	testLlmRewrite,
	testRewriteWithPrompt,
	testSttLastAudio,
}: UsePromptSettingsTestsOptions): PromptSettingsTestsState {
	const [rewriteTestInput, setRewriteTestInput] = useState<string>("");
	const [rewriteTestOutput, setRewriteTestOutput] = useState<string>("");
	const [rewriteTestError, setRewriteTestError] = useState<string>("");
	const [rewriteTestDurationMs, setRewriteTestDurationMs] = useState<
		number | null
	>(null);
	const rewriteTestStartRef = useRef<number | null>(null);

	const runRewriteTest = (promptOverride?: string) => {
		setRewriteTestError("");
		setRewriteTestOutput("");
		setRewriteTestDurationMs(null);
		rewriteTestStartRef.current = performance.now();

		if (typeof promptOverride === "string") {
			testRewriteWithPrompt.mutate(
				{
					transcript: rewriteTestInput,
					prompt: promptOverride,
					profileId: activeProfileId,
				},
				{
					onSuccess: (res) => {
						const startedAt = rewriteTestStartRef.current;
						rewriteTestStartRef.current = null;
						if (typeof startedAt === "number") {
							setRewriteTestDurationMs(performance.now() - startedAt);
						}
						setRewriteTestOutput(res.output);
					},
					onError: (err) => {
						const startedAt = rewriteTestStartRef.current;
						rewriteTestStartRef.current = null;
						if (typeof startedAt === "number") {
							setRewriteTestDurationMs(performance.now() - startedAt);
						}
						setRewriteTestError(errorToMessage(err));
					},
				},
			);
			return;
		}

		testLlmRewrite.mutate(
			{
				transcript: rewriteTestInput,
				profileId: activeProfileId,
			},
			{
				onSuccess: (res) => {
					const startedAt = rewriteTestStartRef.current;
					rewriteTestStartRef.current = null;
					if (typeof startedAt === "number") {
						setRewriteTestDurationMs(performance.now() - startedAt);
					}
					setRewriteTestOutput(res.output);
				},
				onError: (err) => {
					const startedAt = rewriteTestStartRef.current;
					rewriteTestStartRef.current = null;
					if (typeof startedAt === "number") {
						setRewriteTestDurationMs(performance.now() - startedAt);
					}
					setRewriteTestError(errorToMessage(err));
				},
			},
		);
	};

	const [sttTestOutput, setSttTestOutput] = useState<string>("");
	const [sttTestError, setSttTestError] = useState<string>("");
	const [sttTestDurationMs, setSttTestDurationMs] = useState<number | null>(
		null,
	);
	const sttTestStartRef = useRef<number | null>(null);

	const handleRunSttTest = () => {
		setSttTestError("");
		setSttTestOutput("");
		setSttTestDurationMs(null);
		sttTestStartRef.current = performance.now();

		testSttLastAudio.mutate(
			{
				profileId: activeProfileId,
			},
			{
				onSuccess: (res) => {
					const startedAt = sttTestStartRef.current;
					sttTestStartRef.current = null;
					if (typeof startedAt === "number") {
						setSttTestDurationMs(performance.now() - startedAt);
					}

					setSttTestOutput(res);
				},
				onError: (err) => {
					const startedAt = sttTestStartRef.current;
					sttTestStartRef.current = null;
					if (typeof startedAt === "number") {
						setSttTestDurationMs(performance.now() - startedAt);
					}

					setSttTestError(errorToMessage(err));
				},
			},
		);
	};

	return {
		rewriteTestInput,
		setRewriteTestInput,
		rewriteTestOutput,
		rewriteTestError,
		rewriteTestDurationMs,
		runRewriteTest,
		sttTestOutput,
		sttTestError,
		sttTestDurationMs,
		handleRunSttTest,
	};
}
