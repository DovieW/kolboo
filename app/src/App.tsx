import { NavLink, Text, Tooltip } from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
	BarChart2,
	FileAudio,
	FileText,
	Home,
	Settings,
	UserRound,
} from "lucide-react";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import appPackageJson from "../package.json";
import { AccountView } from "./components/account";
import { FileTranscription } from "./components/FileTranscription";
import { HistoryFeed } from "./components/HistoryFeed";
import { Logo } from "./components/Logo";
import { LogsView } from "./components/LogsView";
import { MicStatusCard } from "./components/MicStatusCard";
import { RecordingBar } from "./components/RecordingBar";
import { SettingsShell } from "./components/settings";
import { SettingsGuideOverlay } from "./components/settings/SettingsGuideOverlay";
import { TelemetryDisclosureModal } from "./components/settings/TelemetryDisclosureModal";
import { UsageStatsView } from "./components/usageStats/UsageStatsView";
import { useModifierKeyForwarder } from "./hooks/useModifierKeyForwarder";
import { applyAccentColor } from "./lib/accentColor";
import {
	readBootAccentColor,
	readBootGuideState,
	setBootGuideState,
} from "./lib/bootStorage";
import { frontendLog } from "./lib/frontendLog";
import {
	useSetSettingsGuideState,
	useSettings,
	useSettingsGuideState,
} from "./lib/queries";
import { isTelemetryDisclosureResolved } from "./lib/settings/telemetryDisclosure";
import { getPolicyPathEnforcement, tauriAPI } from "./lib/tauri";
import { listenTyped } from "./lib/tauri/events";
import { AggregateAnalyticsBridge } from "./lib/telemetry/AggregateAnalyticsBridge";
import {
	checkSignedUpdateVersion,
	compareSemver,
	installSignedUpdate,
	signedUpdaterEnabled,
} from "./lib/updates";
import "./styles.css";

type View =
	| "home"
	| "transcribe-file"
	| "settings"
	| "logs"
	| "usage-stats"
	| "account";

function Sidebar({
	activeView,
	onViewChange,
}: {
	activeView: View;
	onViewChange: (view: View) => void;
}) {
	const currentVersion = appPackageJson.version;

	const { data: latestReleaseVersion } = useQuery({
		queryKey: ["signedUpdateVersion"],
		queryFn: checkSignedUpdateVersion,
		enabled: signedUpdaterEnabled,
		staleTime: 6 * 60 * 60 * 1000,
		refetchOnWindowFocus: false,
		retry: false,
	});

	const updateAvailable =
		typeof latestReleaseVersion === "string" &&
		compareSemver(latestReleaseVersion, currentVersion) > 0;

	const releaseUrl = "https://github.com/DovieW/kolboo/releases";
	const installUpdate = async () => {
		try {
			await installSignedUpdate();
		} catch (error) {
			notifications.show({
				title: "Update failed",
				message: error instanceof Error ? error.message : "Try again later.",
				color: "red",
			});
		}
	};

	return (
		<aside className="sidebar" aria-label="Kolboo">
			<header className="sidebar-header">
				<div className="sidebar-logo">
					<Logo size={32} />
					<span className="sidebar-brand">Kolboo</span>
				</div>
			</header>

			<nav className="sidebar-nav" aria-label="Main navigation">
				{(
					[
						{ view: "home", label: "Home", icon: Home },
						{
							view: "transcribe-file",
							label: "Transcribe file",
							icon: FileAudio,
						},
						{ view: "settings", label: "Settings", icon: Settings },
						{ view: "usage-stats", label: "Usage", icon: BarChart2 },
						{ view: "account", label: "Account", icon: UserRound },
						{ view: "logs", label: "Logs", icon: FileText },
					] as const
				).map(({ view, label, icon: Icon }) => (
					<Tooltip key={view} label={label} position="right" withArrow>
						<NavLink
							component="button"
							type="button"
							label={label}
							aria-label={label}
							aria-current={activeView === view ? "page" : undefined}
							leftSection={
								<Icon size={20} strokeWidth={1.5} aria-hidden="true" />
							}
							active={activeView === view}
							onClick={() => onViewChange(view)}
							variant="filled"
							className="sidebar-nav-link"
						/>
					</Tooltip>
				))}
			</nav>

			<footer className="sidebar-footer">
				{updateAvailable ? (
					<Tooltip
						label={
							<Text size="xs" fw={700}>
								UPDATE
							</Text>
						}
						withArrow
						position="top"
						offset={6}
						arrowSize={6}
						radius="sm"
						color="red"
						opened
					>
						<a
							className="sidebar-footer-link"
							href={releaseUrl}
							onClick={(event) => {
								if (!signedUpdaterEnabled) return;
								event.preventDefault();
								void installUpdate();
							}}
						>
							v{currentVersion}
						</a>
					</Tooltip>
				) : (
					<a
						className="sidebar-footer-link"
						href={releaseUrl}
						target="_blank"
						rel="noreferrer"
					>
						v{currentVersion}
					</a>
				)}
			</footer>
		</aside>
	);
}

function HomeView({ onJumpToLog }: { onJumpToLog?: (logId: string) => void }) {
	return (
		<div className="main-content home-page">
			<div className="main-content-inner page-content-start">
				<MicStatusCard />
				<HistoryFeed onJumpToLog={onJumpToLog} />
				<RecordingBar />
				<div aria-hidden="true" style={{ height: 80 }} />
			</div>
		</div>
	);
}

function SettingsViewWithGuideLauncher({
	onRunSetupGuide,
}: {
	onRunSetupGuide: () => void;
}) {
	return <SettingsShell onRunSetupGuide={onRunSetupGuide} />;
}

function AccentColorSync() {
	const { data: settings } = useSettings();

	// Read once so Ctrl+R can apply the user's accent immediately, without waiting
	// for the async Tauri store to hydrate.
	const bootAccent = useMemo(() => readBootAccentColor(), []);

	// Use layout effect so this runs before paint (avoids a one-frame accent flash).
	useLayoutEffect(() => {
		const effectiveAccent = settings ? settings.accent_color : bootAccent;
		applyAccentColor(effectiveAccent);
	}, [bootAccent, settings]);

	return null;
}

export default function App() {
	// Forward modifier-only key events (like AltRight) to the backend.
	// WebView2 intercepts these before our keyboard hook sees them.
	useModifierKeyForwarder();

	const queryClient = useQueryClient();

	const bootGuideState = readBootGuideState();
	const bootGuideKnown = bootGuideState !== null;
	const bootShouldAutoOpenGuide = bootGuideState === "pending";

	frontendLog.info(
		"boot",
		`guideState=${bootGuideState} known=${bootGuideKnown} autoOpen=${bootShouldAutoOpenGuide}`,
	);

	// Safety valve: on some fresh installs, the first attempt to read the Tauri store
	// (via plugin-store) can error or never resolve until the webview is reloaded.
	// We should never show an infinite blank/black window; after a short grace
	// period, fall back to opening the setup guide.
	const [bootGuideFallbackActivated, setBootGuideFallbackActivated] =
		useState(false);

	const [activeView, setActiveView] = useState<View>(() =>
		bootShouldAutoOpenGuide ? "settings" : "home",
	);
	const [logsJumpToId, setLogsJumpToId] = useState<string | null>(null);
	const [settingsGuideOpen, setSettingsGuideOpen] = useState<boolean>(
		() => bootShouldAutoOpenGuide,
	);
	const lastSingleInstanceToastAt = useRef(0);

	const guideQuery = useSettingsGuideState();
	const guideState = guideQuery.data;
	const setGuideState = useSetSettingsGuideState();
	const { data: settings } = useSettings();
	const resolveTelemetryDisclosure = useMutation({
		mutationFn: (analyticsEnabled: boolean) =>
			tauriAPI.resolveTelemetryDisclosure({ analyticsEnabled }),
		onSuccess: async () => {
			await queryClient.invalidateQueries({ queryKey: ["settings"] });
			await queryClient.invalidateQueries({ queryKey: ["cloudSyncUiState"] });
		},
	});
	const telemetryDisclosureResolved = settings
		? isTelemetryDisclosureResolved({
				telemetryDisclosureAcknowledgedAt:
					settings.telemetry_disclosure_acknowledged_at,
				telemetryDisclosureVersion: settings.telemetry_disclosure_version,
			})
		: false;
	const analyticsPolicy = getPolicyPathEnforcement(
		settings?.policy_state,
		"posthog_analytics_enabled",
	);
	const shouldShowTelemetryDisclosure =
		!settingsGuideOpen && Boolean(settings) && !telemetryDisclosureResolved;

	// The optional per-installation toggle does not govern aggregate counts.
	const aggregateAnalyticsEnabled = telemetryDisclosureResolved;

	// Keep the cost summary cache in sync even when the Stats view isn't mounted.
	useEffect(() => {
		let unlisten: (() => void) | null = null;

		tauriAPI
			.onStatsChanged(() => {
				queryClient.invalidateQueries({ queryKey: ["costSummary"] });
				queryClient.invalidateQueries({ queryKey: ["costByProvider"] });
			})
			.then((fn) => {
				unlisten = fn;
			})
			.catch((e) => {
				console.warn("Failed to subscribe to stats-changed:", e);
			});

		return () => {
			try {
				unlisten?.();
			} catch {
				// ignore
			}
		};
	}, [queryClient]);

	useEffect(() => {
		let unlisten: (() => void) | null = null;

		tauriAPI
			.onTranscriptCopiedToClipboard(() => {
				notifications.show({
					title: "Copied to clipboard",
					message:
						"Transcript was copied because the app couldn't safely insert it.",
					color: "orange",
				});
			})
			.then((fn) => {
				unlisten = fn;
			})
			.catch((e) => {
				console.warn("Failed to subscribe to clipboard fallback:", e);
			});

		return () => {
			try {
				unlisten?.();
			} catch {
				// ignore
			}
		};
	}, []);

	useEffect(() => {
		let unlisten: (() => void) | null = null;

		listenTyped("single-instance-activated", () => {
			const now = Date.now();
			if (now - lastSingleInstanceToastAt.current < 1500) {
				return;
			}
			lastSingleInstanceToastAt.current = now;
			notifications.show({
				title: "Already running",
				message: "Kolboo is already running.",
				color: "blue",
			});
		})
			.then((fn) => {
				unlisten = fn;
			})
			.catch((e) => {
				console.warn("Failed to subscribe to single-instance-activated:", e);
			});

		return () => {
			try {
				unlisten?.();
			} catch {
				// ignore
			}
		};
	}, []);

	useEffect(() => {
		if (bootGuideKnown) return;
		if (guideState !== undefined) return;
		if (guideQuery.isError) return;
		if (bootGuideFallbackActivated) return;

		const t = window.setTimeout(() => {
			frontendLog.warn(
				"boot",
				"Boot guide fallback activated (Tauri store read timed out or failed)",
			);
			setBootGuideFallbackActivated(true);
			// Also seed localStorage so subsequent reloads / first-paint logic can
			// immediately decide to open the guide.
			setBootGuideState("pending");
		}, 1200);

		return () => {
			window.clearTimeout(t);
		};
	}, [
		bootGuideFallbackActivated,
		bootGuideKnown,
		guideQuery.isError,
		guideState,
	]);

	useEffect(() => {
		// If the guide state failed to load (or timed out), treat it like a first-run
		// and open the setup guide instead of leaving the user on a blank page.
		if (bootGuideKnown) return;
		if (guideState !== undefined) return;
		if (!guideQuery.isError && !bootGuideFallbackActivated) return;

		setActiveView("settings");
		setSettingsGuideOpen(true);
	}, [
		bootGuideFallbackActivated,
		bootGuideKnown,
		guideQuery.isError,
		guideState,
	]);

	useEffect(() => {
		if (guideState === "pending") {
			setActiveView("settings");
			setSettingsGuideOpen(true);
		}
	}, [guideState]);

	// If we don't have a boot hint yet (first ever run, or storage was cleared),
	// avoid rendering the Home view for a moment before the guide state arrives.
	// IMPORTANT: do not early-return before hooks/effects are declared.
	const shouldShowBootSplash =
		!bootGuideKnown &&
		guideState === undefined &&
		!guideQuery.isError &&
		!bootGuideFallbackActivated;

	const renderView = () => {
		switch (activeView) {
			case "home":
				return (
					<HomeView
						onJumpToLog={(logId) => {
							setLogsJumpToId(logId);
							setActiveView("logs");
						}}
					/>
				);
			case "transcribe-file":
				return null;
			case "settings":
				return (
					<SettingsViewWithGuideLauncher
						onRunSetupGuide={() => {
							setSettingsGuideOpen(true);
						}}
					/>
				);
			case "logs":
				return (
					<LogsView
						jumpToLogId={logsJumpToId}
						onJumpHandled={() => setLogsJumpToId(null)}
					/>
				);
			case "usage-stats":
				return <UsageStatsView />;
			case "account":
				return <AccountView />;
			default:
				return (
					<HomeView
						onJumpToLog={(logId) => {
							setLogsJumpToId(logId);
							setActiveView("logs");
						}}
					/>
				);
		}
	};

	if (shouldShowBootSplash) {
		return (
			<div
				style={{
					position: "fixed",
					inset: 0,
					background: "#0b0d10",
				}}
			/>
		);
	}

	return (
		<div className="app-layout">
			<AggregateAnalyticsBridge
				enabled={aggregateAnalyticsEnabled}
				page={activeView}
			/>
			<a className="skip-navigation" href="#main-content">
				Skip to content
			</a>
			<AccentColorSync />
			<Sidebar
				activeView={activeView}
				onViewChange={(view) => {
					setLogsJumpToId(null);
					setActiveView(view);
					if (view === "settings" && guideState === "pending") {
						setSettingsGuideOpen(true);
					}
				}}
			/>
			<main id="main-content" className="app-page" tabIndex={-1}>
				{renderView()}
				<div
					className="retained-page"
					hidden={activeView !== "transcribe-file"}
				>
					<FileTranscription
						active={activeView === "transcribe-file"}
						onOpenHistory={() => setActiveView("home")}
					/>
				</div>
			</main>

			<SettingsGuideOverlay
				opened={settingsGuideOpen}
				onSkip={() => {
					setSettingsGuideOpen(false);
					setGuideState.mutate("skipped");
				}}
				onFinished={() => {
					setSettingsGuideOpen(false);
					setGuideState.mutate("completed");
				}}
				onGoHome={() => {
					setActiveView("home");
				}}
			/>

			<TelemetryDisclosureModal
				opened={shouldShowTelemetryDisclosure}
				analyticsPolicyEnforced={analyticsPolicy.enforced}
				analyticsPolicyReason={analyticsPolicy.reason}
				loading={resolveTelemetryDisclosure.isPending}
				onDisableAnalytics={() => {
					resolveTelemetryDisclosure.mutate(false);
				}}
				onAllowAnalytics={() => {
					resolveTelemetryDisclosure.mutate(true);
				}}
			/>
		</div>
	);
}
