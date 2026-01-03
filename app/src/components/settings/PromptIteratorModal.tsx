import {
  Button,
  Group,
  Loader,
  Modal,
  Stack,
  Text,
  Textarea,
} from "@mantine/core";
import { useEffect, useState } from "react";
import { useImprovePrompt } from "../../lib/queries";

interface PromptIteratorModalProps {
  opened: boolean;
  onClose: () => void;
  currentPrompt: string;
  profileId?: string;
  onApplyPrompt?: (improvedPrompt: string) => void;
}

export function PromptIteratorModal({
  opened,
  onClose,
  currentPrompt,
  profileId,
  onApplyPrompt,
}: PromptIteratorModalProps) {
  const [input, setInput] = useState("");
  const [actualOutput, setActualOutput] = useState("");
  const [desiredOutput, setDesiredOutput] = useState("");
  const [reasoning, setReasoning] = useState("");
  const [improvedPrompt, setImprovedPrompt] = useState("");
  const [testOutput, setTestOutput] = useState("");
  const [testError, setTestError] = useState("");
  const [isTestingPrompt, setIsTestingPrompt] = useState(false);

  const improvePrompt = useImprovePrompt();

  // Reset form when modal opens/closes
  useEffect(() => {
    if (!opened) {
      setInput("");
      setActualOutput("");
      setDesiredOutput("");
      setReasoning("");
      setImprovedPrompt("");
      setTestOutput("");
      setTestError("");
    }
  }, [opened]);

  const handleImprovePrompt = () => {
    setImprovedPrompt("");
    setTestOutput("");
    setTestError("");

    improvePrompt.mutate(
      {
        currentPrompt,
        input,
        actualOutput,
        desiredOutput,
        reasoning: reasoning.trim() || null,
        profileId: profileId ?? null,
      },
      {
        onSuccess: (response) => {
          setImprovedPrompt(response.improvedPrompt);
        },
        onError: (err: any) => {
          setTestError(
            err?.message || err?.toString() || "Failed to improve prompt"
          );
        },
      }
    );
  };

  const handleTestPrompt = async () => {
    if (!improvedPrompt.trim()) return;

    setTestOutput("");
    setTestError("");
    setIsTestingPrompt(true);

    try {
      // Testing with the improved prompt would require temporarily applying it
      // or implementing a custom test command that accepts a prompt override.
      // For now, we show a helpful message.
      await new Promise((resolve) => setTimeout(resolve, 500));
      setTestOutput(
        "To test this improved prompt:\n\n" +
        "1. Click 'Apply to Settings' to use it\n" +
        "2. Then use the 'Test rewrite' section above to validate the results\n\n" +
        "This ensures you're testing with the exact same configuration."
      );
    } catch (err: any) {
      setTestError(err?.message || err?.toString() || "Test failed");
    } finally {
      setIsTestingPrompt(false);
    }
  };

  const handleApplyPrompt = () => {
    if (improvedPrompt.trim() && onApplyPrompt) {
      onApplyPrompt(improvedPrompt.trim());
      onClose();
    }
  };

  const canImprove =
    input.trim().length > 0 &&
    actualOutput.trim().length > 0 &&
    desiredOutput.trim().length > 0 &&
    !improvePrompt.isPending;

  const canTest =
    improvedPrompt.trim().length > 0 &&
    input.trim().length > 0 &&
    !isTestingPrompt;

  return (
    <Modal
      opened={opened}
      onClose={onClose}
      title="Prompt Iterator"
      size="xl"
      centered
      styles={{
        content: {
          height: "min(900px, 85vh)",
          display: "flex",
          flexDirection: "column",
          overflow: "hidden",
        },
        body: {
          flex: 1,
          display: "flex",
          flexDirection: "column",
          overflow: "hidden",
        },
      }}
    >
      <Stack
        gap="md"
        style={{
          flex: 1,
          minHeight: 0,
          overflow: "auto",
          paddingRight: 8,
        }}
      >
        <div>
          <Text size="xs" c="dimmed" mb={6}>
            Use this tool to improve your prompt by providing examples of what
            you want. The AI will analyze the difference between actual and
            desired output to generate a better prompt.
          </Text>
        </div>

        <div>
          <Text size="sm" fw={500} mb={4}>
            Current Prompt
          </Text>
          <Textarea
            value={currentPrompt}
            readOnly
            autosize
            minRows={2}
            maxRows={4}
            styles={{
              input: {
                backgroundColor: "var(--bg-elevated)",
                borderColor: "var(--border-default)",
                color: "var(--text-muted)",
                fontFamily: "monospace",
                fontSize: "12px",
              },
            }}
          />
        </div>

        <div>
          <Text size="sm" fw={500} mb={4}>
            Input <Text span c="red">*</Text>
          </Text>
          <Textarea
            value={input}
            onChange={(e) => setInput(e.currentTarget.value)}
            placeholder="The raw transcript that was given to the LLM"
            autosize
            minRows={2}
            maxRows={5}
            styles={{
              input: {
                backgroundColor: "var(--bg-elevated)",
                borderColor: "var(--border-default)",
                color: "var(--text-primary)",
                fontFamily: "monospace",
                fontSize: "13px",
              },
            }}
          />
        </div>

        <div>
          <Text size="sm" fw={500} mb={4}>
            Actual Output <Text span c="red">*</Text>
          </Text>
          <Textarea
            value={actualOutput}
            onChange={(e) => setActualOutput(e.currentTarget.value)}
            placeholder="The incorrect/undesired output you got from the rewrite"
            autosize
            minRows={2}
            maxRows={5}
            styles={{
              input: {
                backgroundColor: "var(--bg-elevated)",
                borderColor: "var(--border-default)",
                color: "var(--text-primary)",
                fontFamily: "monospace",
                fontSize: "13px",
              },
            }}
          />
        </div>

        <div>
          <Text size="sm" fw={500} mb={4}>
            Desired Output <Text span c="red">*</Text>
          </Text>
          <Textarea
            value={desiredOutput}
            onChange={(e) => setDesiredOutput(e.currentTarget.value)}
            placeholder="What you wanted the output to be"
            autosize
            minRows={2}
            maxRows={5}
            styles={{
              input: {
                backgroundColor: "var(--bg-elevated)",
                borderColor: "var(--border-default)",
                color: "var(--text-primary)",
                fontFamily: "monospace",
                fontSize: "13px",
              },
            }}
          />
        </div>

        <div>
          <Text size="sm" fw={500} mb={4}>
            Reasoning (Optional)
          </Text>
          <Textarea
            value={reasoning}
            onChange={(e) => setReasoning(e.currentTarget.value)}
            placeholder="Explain why the actual output was incorrect or what specifically you didn't like about it"
            autosize
            minRows={2}
            maxRows={4}
            styles={{
              input: {
                backgroundColor: "var(--bg-elevated)",
                borderColor: "var(--border-default)",
                color: "var(--text-primary)",
                fontFamily: "monospace",
                fontSize: "13px",
              },
            }}
          />
        </div>

        <Group justify="space-between">
          <Button
            color="orange"
            onClick={handleImprovePrompt}
            disabled={!canImprove}
            loading={improvePrompt.isPending}
          >
            Generate Improved Prompt
          </Button>
        </Group>

        {improvedPrompt && (
          <>
            <div>
              <Text size="sm" fw={500} mb={4} c="green">
                Improved Prompt
              </Text>
              <Textarea
                value={improvedPrompt}
                onChange={(e) => setImprovedPrompt(e.currentTarget.value)}
                autosize
                minRows={3}
                maxRows={8}
                styles={{
                  input: {
                    backgroundColor: "var(--bg-elevated)",
                    borderColor: "var(--border-success)",
                    color: "var(--text-primary)",
                    fontFamily: "monospace",
                    fontSize: "13px",
                  },
                }}
              />
            </div>

            <Group justify="space-between">
              <Button
                color="gray"
                onClick={handleTestPrompt}
                disabled={!canTest}
                loading={isTestingPrompt}
              >
                Test Improved Prompt
              </Button>
              {onApplyPrompt && (
                <Button
                  color="green"
                  onClick={handleApplyPrompt}
                  disabled={!improvedPrompt.trim()}
                >
                  Apply to Settings
                </Button>
              )}
            </Group>

            {testOutput && (
              <div>
                <Text size="sm" fw={500} mb={4}>
                  Test Result
                </Text>
                <Textarea
                  value={testOutput}
                  readOnly
                  autosize
                  minRows={2}
                  maxRows={6}
                  styles={{
                    input: {
                      backgroundColor: "var(--bg-elevated)",
                      borderColor: "var(--border-default)",
                      color: "var(--text-primary)",
                      fontFamily: "monospace",
                      fontSize: "13px",
                    },
                  }}
                />
              </div>
            )}
          </>
        )}

        {testError && (
          <Text size="sm" c="red">
            {testError}
          </Text>
        )}
      </Stack>
    </Modal>
  );
}
