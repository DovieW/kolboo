import { Button, Text, Textarea } from "@mantine/core";

type TestRewritePanelProps = {
	header: string;
	inputValue: string;
	onInputChange: (value: string) => void;
	onRun: () => void;
	isRunning: boolean;
	durationMs: number | null;
	error: string;
	output: string;
	isDisabled: boolean;
	inputPlaceholder?: string;
};

function formatDuration(durationMs: number | null, isRunning: boolean): string {
	if (isRunning) return "Duration: running…";
	if (durationMs === null) return "Duration: —";
	return `Duration: ${(durationMs / 1000).toFixed(2)}s`;
}

export function TestRewritePanel({
	header,
	inputValue,
	onInputChange,
	onRun,
	isRunning,
	durationMs,
	error,
	output,
	isDisabled,
	inputPlaceholder,
}: TestRewritePanelProps) {
	return (
		<div
			style={{
				display: "flex",
				flexDirection: "column",
				gap: 10,
			}}
		>
			<Text size="xs" c="dimmed">
				{header}
			</Text>

			<Textarea
				value={inputValue}
				onChange={(e) => {
					onInputChange(e.currentTarget.value);
				}}
				onKeyDown={(e) => {
					if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
						e.preventDefault();
						if (!isDisabled) {
							onRun();
						}
					}
				}}
				placeholder={inputPlaceholder}
				autosize
				minRows={3}
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

			<div
				style={{
					display: "flex",
					alignItems: "center",
					gap: 12,
				}}
			>
				<Button
					color="gray"
					loading={isRunning}
					disabled={isDisabled}
					onClick={onRun}
				>
					Test
				</Button>

				<Text size="sm" c="dimmed">
					{formatDuration(durationMs, isRunning)}
				</Text>
			</div>

			{error ? (
				<Text size="sm" c="red">
					{error}
				</Text>
			) : null}

			{output ? (
				<Textarea
					value={output}
					readOnly
					autosize
					minRows={3}
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
			) : null}
		</div>
	);
}
