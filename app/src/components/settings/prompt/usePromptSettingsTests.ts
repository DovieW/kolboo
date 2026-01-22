import {
	type Dispatch,
	type MutableRefObject,
	type SetStateAction,
	useRef,
	useState,
} from "react";

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
	const finishTimer = (
		startRef: MutableRefObject<number | null>,
		setDurationMs: Dispatch<SetStateAction<number | null>>,
	) => {
		const startedAt = startRef.current;
		startRef.current = null;
		if (typeof startedAt === "number") {
			setDurationMs(performance.now() - startedAt);
		}
	};

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
						finishTimer(rewriteTestStartRef, setRewriteTestDurationMs);
						setRewriteTestOutput(res.output);
					},
					onError: (err) => {
						finishTimer(rewriteTestStartRef, setRewriteTestDurationMs);
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
					finishTimer(rewriteTestStartRef, setRewriteTestDurationMs);
					setRewriteTestOutput(res.output);
				},
				onError: (err) => {
					finishTimer(rewriteTestStartRef, setRewriteTestDurationMs);
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
					finishTimer(sttTestStartRef, setSttTestDurationMs);

					setSttTestOutput(res);
				},
				onError: (err) => {
					finishTimer(sttTestStartRef, setSttTestDurationMs);

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
