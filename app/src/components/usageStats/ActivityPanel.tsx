import {
	Alert,
	Button,
	Group,
	Paper,
	SegmentedControl,
	SimpleGrid,
	Skeleton,
	Stack,
	Table,
	Text,
	Tooltip,
} from "@mantine/core";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import type { CostTimeframe } from "../../lib/tauri";
import { tauriAPI } from "../../lib/tauri";
import { listenTyped } from "../../lib/tauri/events";
import { activitySeries, formatAudioTime } from "./activity";

export function ActivityPanel({
	timeframe,
	modelsOnly = false,
}: {
	timeframe: CostTimeframe;
	modelsOnly?: boolean;
}) {
	const client = useQueryClient();
	const activity = useQuery({
		queryKey: ["historyActivity", timeframe],
		queryFn: () => tauriAPI.getHistoryActivity(timeframe),
		refetchOnMount: "always",
	});
	const [metric, setMetric] = useState("words");
	useEffect(() => {
		let disposed = false;
		let stop: (() => void) | undefined;
		void listenTyped("history-changed", () => {
			void client.invalidateQueries({ queryKey: ["historyActivity"] });
		})
			.then((unlisten) => {
				if (disposed) unlisten();
				else stop = unlisten;
			})
			.catch(() => {});
		return () => {
			disposed = true;
			stop?.();
		};
	}, [client]);
	if (activity.isPending)
		return (
			<Stack gap="lg">
				<Skeleton height={110} />
				<Skeleton height={260} />
			</Stack>
		);
	if (activity.isError)
		return (
			<Alert color="red">
				Couldn’t load activity.{" "}
				<Button
					variant="subtle"
					size="compact-xs"
					onClick={() => void activity.refetch()}
				>
					Retry
				</Button>
			</Alert>
		);
	const { totals, days, models } = activity.data;
	const number = (value: number) => value.toLocaleString();
	const series = activitySeries(days, timeframe);
	const key = metric as "words" | "audio_seconds" | "recordings";
	const maximum = Math.max(1, ...series.map((bucket) => bucket[key]));
	return (
		<Stack gap="lg">
			{!modelsOnly && (
				<>
					<SimpleGrid cols={{ base: 2, sm: 4 }} spacing="md">
						{[
							["Words transcribed", number(totals.words)],
							["Audio transcribed", formatAudioTime(totals.audio_seconds)],
							["Recordings", number(totals.recordings)],
							["Active days", number(days.length)],
						].map(([label, value]) => (
							<Paper key={label} withBorder radius="md" p="lg">
								<Text size="sm" c="dimmed">
									{label}
								</Text>
								<Text className="usage-number">{value}</Text>
							</Paper>
						))}
					</SimpleGrid>
					<Paper withBorder radius="md" p="lg">
						<Group justify="space-between" mb="xl">
							<Text fw={500}>Over time</Text>
							<SegmentedControl
								aria-label="Activity metric"
								size="xs"
								value={metric}
								onChange={setMetric}
								data={[
									{ value: "words", label: "Words" },
									{ value: "audio_seconds", label: "Audio" },
									{ value: "recordings", label: "Recordings" },
								]}
							/>
						</Group>
						{totals.recordings ? (
							<>
								<figure className="usage-chart" aria-label="Activity over time">
									{series.map((bucket) => {
										const value =
											key === "audio_seconds"
												? formatAudioTime(bucket[key])
												: number(bucket[key]);
										const label = `${bucket.date}${bucket.date === bucket.endDate ? "" : ` – ${bucket.endDate}`}: ${value}${key === "audio_seconds" ? " audio" : ` ${metric}`}`;
										return (
											<Tooltip
												key={bucket.date}
												label={label}
												events={{ hover: true, focus: true, touch: true }}
												withArrow
											>
												<div
													className="usage-bar-column"
													role="img"
													aria-label={label}
												>
													<div
														className="usage-bar"
														style={{
															height: `${Math.max(1, (bucket[key] / maximum) * 100)}%`,
															opacity: bucket[key] ? 1 : 0.2,
														}}
													/>
												</div>
											</Tooltip>
										);
									})}
								</figure>
								<Group justify="space-between" mt="sm">
									<Text size="xs" c="dimmed">
										{series[0]?.date}
									</Text>
									<Text size="xs" c="dimmed">
										UTC · {series.at(-1)?.endDate}
									</Text>
								</Group>
							</>
						) : (
							<Text c="dimmed" ta="center" py={70}>
								Your activity will appear here after a transcription.
							</Text>
						)}
					</Paper>
					<Group justify="space-between">
						<Text size="sm" c="dimmed">
							{number(totals.recordings - totals.meetings)} dictations ·{" "}
							{number(totals.meetings)} meetings & files
						</Text>
						{totals.timed_recordings < totals.recordings && (
							<Text size="xs" c="dimmed">
								Audio duration available for {number(totals.timed_recordings)}{" "}
								recordings
							</Text>
						)}
					</Group>
				</>
			)}
			{modelsOnly && (
				<Paper withBorder radius="md" p="lg">
					<Table.ScrollContainer minWidth={500}>
						<Table verticalSpacing="md">
							<Table.Thead>
								<Table.Tr>
									<Table.Th>Model</Table.Th>
									<Table.Th ta="right">Recordings</Table.Th>
									<Table.Th ta="right">Words</Table.Th>
									<Table.Th ta="right">Audio</Table.Th>
								</Table.Tr>
							</Table.Thead>
							<Table.Tbody>
								{models.map((model) => (
									<Table.Tr key={`${model.provider}::${model.model}`}>
										<Table.Td>
											<Text size="sm">{model.model}</Text>
											<Text size="xs" c="dimmed">
												{model.provider}
											</Text>
										</Table.Td>
										<Table.Td ta="right">
											{number(model.totals.recordings)}
										</Table.Td>
										<Table.Td ta="right">{number(model.totals.words)}</Table.Td>
										<Table.Td ta="right">
											{formatAudioTime(model.totals.audio_seconds)}
										</Table.Td>
									</Table.Tr>
								))}
							</Table.Tbody>
						</Table>
						{!models.length && (
							<Text c="dimmed" ta="center" py="xl">
								No transcriptions in this period.
							</Text>
						)}
					</Table.ScrollContainer>
				</Paper>
			)}
			<Text size="xs" c="dimmed">
				From saved history on this device. Reruns count once.
			</Text>
		</Stack>
	);
}
