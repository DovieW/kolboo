import {
	ActionIcon,
	Button,
	Checkbox,
	Group,
	Indicator,
	MultiSelect,
	Popover,
	SegmentedControl,
	Select,
	Stack,
	Tabs,
	Text,
} from "@mantine/core";
import { Filter } from "lucide-react";
import { useState } from "react";
import {
	listAllLlmModelKeys,
	listAllSttModelKeys,
} from "../../lib/modelOptions";
import { useLicenseState } from "../../lib/queries/license";
import type { CostTimeframe } from "../../lib/tauri";
import { ActivityPanel } from "./ActivityPanel";
import { CostTab, type StatsKindFilter } from "./CostTab";
import "./usage.css";
export function UsageStatsView() {
	const [timeframe, setTimeframe] = useState<CostTimeframe>("30d");
	const [tab, setTab] = useState<string | null>("activity");
	const license = useLicenseState();
	const showSpend = Boolean(
		license.data &&
			(license.data.status === "signed_out" ||
				license.data.tier === "community"),
	);
	const activeTab = tab === "spend" && !showSpend ? "activity" : tab;
	return (
		<div className="main-content usage-page">
			<div className="main-content-inner page-content-start">
				<Tabs value={activeTab} onChange={setTab} keepMounted={false}>
					<Group className="usage-toolbar" justify="space-between" gap="md">
						<Tabs.List aria-label="Usage views">
							<Tabs.Tab value="activity">Activity</Tabs.Tab>
							<Tabs.Tab value="models">Models</Tabs.Tab>
							{showSpend && <Tabs.Tab value="spend">Spend</Tabs.Tab>}
						</Tabs.List>
						<Select
							aria-label="Usage period"
							value={timeframe}
							onChange={(value) => setTimeframe(value as CostTimeframe)}
							allowDeselect={false}
							w={180}
							data={[
								{ value: "24h", label: "Last 24 hours" },
								{ value: "7d", label: "Last 7 days" },
								{ value: "30d", label: "Last 30 days" },
								{ value: "90d", label: "Last 90 days" },
								{ value: "all", label: "All time" },
							]}
						/>
					</Group>
					<Tabs.Panel value="activity">
						<ActivityPanel timeframe={timeframe} />
					</Tabs.Panel>
					<Tabs.Panel value="models">
						<ActivityPanel timeframe={timeframe} modelsOnly />
					</Tabs.Panel>
					{showSpend && (
						<Tabs.Panel value="spend">
							<SpendPanel timeframe={timeframe} />
						</Tabs.Panel>
					)}
				</Tabs>
			</div>
		</div>
	);
}
function SpendPanel({ timeframe }: { timeframe: CostTimeframe }) {
	const [opened, setOpened] = useState(false);
	const [kind, setKind] = useState<StatsKindFilter>("all");
	const [stt, setStt] = useState<string[]>([]);
	const [llm, setLlm] = useState<string[]>([]);
	const [excludeFreeTier, setExcludeFreeTier] = useState(true);
	const filtered =
		kind !== "all" || stt.length > 0 || llm.length > 0 || !excludeFreeTier;
	return (
		<div className="usage-spend">
			<div className="usage-controls">
				<Popover
					opened={opened}
					onChange={setOpened}
					position="bottom-end"
					width={320}
					shadow="md"
				>
					<Popover.Target>
						<Indicator disabled={!filtered} size={7}>
							<ActionIcon
								variant="default"
								size={36}
								aria-label="Filters"
								title="Filters"
								onClick={() => setOpened(!opened)}
							>
								<Filter size={16} />
							</ActionIcon>
						</Indicator>
					</Popover.Target>
					<Popover.Dropdown>
						<Stack gap="md">
							<Group justify="space-between">
								<Text size="sm" fw={500}>
									Filters
								</Text>
								{filtered && (
									<Button
										size="compact-xs"
										variant="subtle"
										onClick={() => {
											setKind("all");
											setStt([]);
											setLlm([]);
											setExcludeFreeTier(true);
										}}
									>
										Reset
									</Button>
								)}
							</Group>
							<SegmentedControl
								value={kind}
								onChange={(value) => setKind(value as StatsKindFilter)}
								size="xs"
								fullWidth
								data={[
									{ value: "all", label: "All" },
									{ value: "stt", label: "Transcription" },
									{ value: "llm", label: "AI text" },
								]}
							/>
							<MultiSelect
								comboboxProps={{ withinPortal: false }}
								label="Transcription models"
								placeholder="All models"
								searchable
								clearable
								value={stt}
								onChange={setStt}
								data={listAllSttModelKeys().map((option) => ({
									value: option.key,
									label: option.label,
								}))}
							/>
							<MultiSelect
								comboboxProps={{ withinPortal: false }}
								label="Text models"
								placeholder="All models"
								searchable
								clearable
								value={llm}
								onChange={setLlm}
								data={listAllLlmModelKeys().map((option) => ({
									value: option.key,
									label: option.label,
								}))}
							/>
							<Checkbox
								label="Exclude free tier"
								checked={excludeFreeTier}
								onChange={(event) =>
									setExcludeFreeTier(event.currentTarget.checked)
								}
							/>
						</Stack>
					</Popover.Dropdown>
				</Popover>
			</div>
			<CostTab
				timeframe={timeframe}
				kind={kind}
				sttModelKeys={stt}
				llmModelKeys={llm}
				excludeFreeTier={excludeFreeTier}
			/>
		</div>
	);
}
