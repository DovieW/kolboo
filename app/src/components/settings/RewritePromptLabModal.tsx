import {
  Button,
  Divider,
  Group,
  Modal,
  SegmentedControl,
  SimpleGrid,
  Stack,
  Text,
  Textarea,
} from "@mantine/core";
import { useEffect, useMemo, useState } from "react";

function errorToMessage(err: unknown): string {
  if (err instanceof Error) return err.message;
  if (typeof err === "string") return err;
  try {
    return JSON.stringify(err);
  } catch {
    return String(err);
  }
}

export function RewritePromptLabModal(props: {
  opened: boolean;
  onClose: () => void;

  profileId: string;
  profileLabel: string;

  // Seed values when opening the modal.
  initialTranscript?: string;
  initialProblemOutput?: string;

  // Computed prompt (edited elsewhere).
  currentPrompt: string;

  // Actions
  onIteratePrompt: (params: {
    profileId: string;
    mode: "fixed" | "new";
    transcript: string;
    problemOutput: string;
    desiredOutput: string;
    currentPrompt: string;
  }) => Promise<{
    improvedPrompt: string;
    providerUsed: string;
    modelUsed: string;
  }>;

  onTestPrompt: (params: {
    profileId: string;
    transcript: string;
    prompt: string;
  }) => Promise<{ output: string; providerUsed: string; modelUsed: string }>;
}) {
  const [mode, setMode] = useState<"fixed" | "new">("fixed");

  const [transcript, setTranscript] = useState("");
  const [problemOutput, setProblemOutput] = useState("");
  const [promptGoal, setPromptGoal] = useState("");
  const [desiredOutput, setDesiredOutput] = useState("");

  const [improvedPrompt, setImprovedPrompt] = useState("");
  const [testedOutput, setTestedOutput] = useState("");

  const [improveError, setImproveError] = useState<string>("");
  const [testError, setTestError] = useState<string>("");

  const [improveMeta, setImproveMeta] = useState<string>("");
  const [testMeta, setTestMeta] = useState<string>("");

  const [isImproving, setIsImproving] = useState(false);
  const [isTesting, setIsTesting] = useState(false);

  // Seed values on open.
  useEffect(() => {
    if (!props.opened) return;

    setMode("fixed");
    setTranscript(props.initialTranscript ?? "");
    setProblemOutput(props.initialProblemOutput ?? "");
    setPromptGoal("");
    setDesiredOutput("");
    setImprovedPrompt("");
    setTestedOutput("");
    setImproveError("");
    setTestError("");
    setImproveMeta("");
    setTestMeta("");
  }, [props.opened, props.initialTranscript, props.initialProblemOutput]);

  // Switching modes changes the meaning of inputs, so clear derived outputs.
  useEffect(() => {
    setImprovedPrompt("");
    setTestedOutput("");
    setImproveError("");
    setTestError("");
    setImproveMeta("");
    setTestMeta("");
  }, [mode]);

  const canImprove = useMemo(() => {
    const hasCoreInputs =
      transcript.trim().length > 0 && desiredOutput.trim().length > 0;
    const hasModeSpecificInputs =
      mode === "fixed"
        ? problemOutput.trim().length > 0 && props.currentPrompt.trim().length > 0
        : promptGoal.trim().length > 0;

    return hasCoreInputs && hasModeSpecificInputs && !isImproving && !isTesting;
  }, [
    desiredOutput,
    isImproving,
    isTesting,
    mode,
    problemOutput,
    promptGoal,
    props.currentPrompt,
    transcript,
  ]);

  const canTest = useMemo(() => {
    return (
      transcript.trim().length > 0 &&
      improvedPrompt.trim().length > 0 &&
      !isTesting
    );
  }, [improvedPrompt, isTesting, transcript]);

  const monospaceStyles = {
    input: {
      backgroundColor: "var(--bg-elevated)",
      borderColor: "var(--border-default)",
      color: "var(--text-primary)",
      fontFamily: "monospace",
      fontSize: "13px",
    },
  } as const;

  return (
    <Modal
      opened={props.opened}
      onClose={() => {
        if (isImproving || isTesting) return;
        props.onClose();
      }}
      title={`Prompt Lab · ${props.profileLabel}`}
      centered
      size="90%"
      styles={{
        body: { paddingTop: 8 },
        content: { maxWidth: 1400 },
      }}
    >
      <Stack gap="sm">
        <Group justify="space-between" align="center" gap="sm">
          <SegmentedControl
            value={mode}
            onChange={(v) => setMode(v as "fixed" | "new")}
            data={[
              { label: "Fix prompt", value: "fixed" },
              { label: "New prompt", value: "new" },
            ]}
            disabled={isImproving || isTesting}
          />
          <Text size="xs" c="dimmed">
            {mode === "fixed"
              ? "Use before/after outputs to improve the existing prompt."
              : "Describe your goal and generate a fresh prompt from scratch."}
          </Text>
        </Group>

        <Divider
          label="Inputs"
          labelPosition="left"
          styles={{
            root: { borderColor: "var(--border-subtle)" },
            label: {
              color: "var(--text-primary)",
              fontSize: 11,
              fontWeight: 600,
              letterSpacing: "0.08em",
              textTransform: "uppercase",
            },
          }}
        />

        <SimpleGrid cols={2} spacing="md" verticalSpacing="md">
          <Textarea
            label="Transcript (input)"
            value={transcript}
            onChange={(e) => setTranscript(e.currentTarget.value)}
            rows={6}
            styles={monospaceStyles}
          />

          {mode === "fixed" ? (
            <Textarea
              label="Current prompt (read-only)"
              value={props.currentPrompt}
              readOnly
              rows={6}
              styles={monospaceStyles}
            />
          ) : (
            <Textarea
              label="Prompt goal / description"
              value={promptGoal}
              onChange={(e) => setPromptGoal(e.currentTarget.value)}
              autosize
              minRows={6}
              placeholder="Describe what the new prompt should accomplish and any rules/constraints to follow."
              styles={monospaceStyles}
            />
          )}

          {mode === "fixed" ? (
            <Textarea
              label="Problem output (what you got)"
              value={problemOutput}
              onChange={(e) => setProblemOutput(e.currentTarget.value)}
              autosize
              minRows={6}
              styles={monospaceStyles}
            />
          ) : (
            <Textarea
              label="Existing prompt (reference)"
              value={props.currentPrompt}
              readOnly
              rows={6}
              styles={monospaceStyles}
            />
          )}

          {mode === "new" ? (
            <Textarea
              label="Desired output (what you want)"
              value={desiredOutput}
              onChange={(e) => setDesiredOutput(e.currentTarget.value)}
              rows={6}
              styles={monospaceStyles}
            />
          ) : (
            <Textarea
              label="Desired output (what you want)"
              value={desiredOutput}
              onChange={(e) => setDesiredOutput(e.currentTarget.value)}
              autosize
              minRows={6}
              styles={monospaceStyles}
            />
          )}
        </SimpleGrid>

        <Group justify="flex-end" gap="sm">
          <Button
            variant="light"
            color="gray"
            onClick={() => {
              setDesiredOutput("");
              setImprovedPrompt("");
              setTestedOutput("");
              setImproveError("");
              setTestError("");
              setImproveMeta("");
              setTestMeta("");
            }}
            disabled={isImproving || isTesting}
          >
            Clear outputs
          </Button>

          <Button
            color="gray"
            loading={isImproving}
            disabled={!canImprove}
            onClick={async () => {
              setImproveError("");
              setImproveMeta("");
              setImprovedPrompt("");
              setTestedOutput("");
              setTestError("");
              setTestMeta("");

              setIsImproving(true);
              try {
                const res = await props.onIteratePrompt({
                  profileId: props.profileId,
                  mode,
                  transcript,
                  problemOutput: mode === "fixed" ? problemOutput : promptGoal,
                  desiredOutput,
                  currentPrompt: props.currentPrompt,
                });
                setImprovedPrompt(res.improvedPrompt);
                setImproveMeta(
                  `${res.providerUsed}${
                    res.modelUsed ? ` / ${res.modelUsed}` : ""
                  }`
                );
              } catch (e) {
                setImproveError(errorToMessage(e));
              } finally {
                setIsImproving(false);
              }
            }}
          >
            {mode === "fixed" ? "Improve prompt" : "Create prompt"}
          </Button>

          <Button
            color="gray"
            loading={isTesting}
            disabled={!canTest}
            onClick={async () => {
              setTestError("");
              setTestMeta("");
              setTestedOutput("");

              setIsTesting(true);
              try {
                const res = await props.onTestPrompt({
                  profileId: props.profileId,
                  transcript,
                  prompt: improvedPrompt,
                });
                setTestedOutput(res.output);
                setTestMeta(
                  `${res.providerUsed}${
                    res.modelUsed ? ` / ${res.modelUsed}` : ""
                  }`
                );
              } catch (e) {
                setTestError(errorToMessage(e));
              } finally {
                setIsTesting(false);
              }
            }}
          >
            Test prompt
          </Button>
        </Group>

        {improveError ? (
          <Text size="sm" c="red">
            {improveError}
          </Text>
        ) : null}

        {improveMeta ? (
          <Text size="xs" c="dimmed">
            Improved prompt generated with: {improveMeta}
          </Text>
        ) : null}

        <Divider
          label="Outputs"
          labelPosition="left"
          styles={{
            root: { borderColor: "var(--border-subtle)" },
            label: {
              color: "var(--text-primary)",
              fontSize: 11,
              fontWeight: 600,
              letterSpacing: "0.08em",
              textTransform: "uppercase",
            },
          }}
        />

        <SimpleGrid cols={2} spacing="md" verticalSpacing="md">
          <Textarea
            label={mode === "fixed" ? "Improved prompt" : "Generated prompt"}
            value={improvedPrompt}
            readOnly
            placeholder={
              mode === "fixed"
                ? "Click “Improve prompt” to generate a candidate prompt."
                : "Click “Create prompt” to generate a candidate prompt."
            }
            autosize
            minRows={8}
            styles={monospaceStyles}
          />

          <Textarea
            label="Output from improved prompt"
            value={testedOutput}
            readOnly
            placeholder="Click “Test prompt” to run the improved prompt on the transcript."
            autosize
            minRows={8}
            styles={monospaceStyles}
          />
        </SimpleGrid>

        {testError ? (
          <Text size="sm" c="red">
            {testError}
          </Text>
        ) : null}

        {testMeta ? (
          <Text size="xs" c="dimmed">
            Tested with: {testMeta}
          </Text>
        ) : null}
      </Stack>
    </Modal>
  );
}
