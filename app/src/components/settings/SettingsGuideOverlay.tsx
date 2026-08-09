import {
	Anchor,
	Badge,
	Button,
	Group,
	Kbd,
	PasswordInput,
	Text,
	Textarea,
	TextInput,
	Title,
} from "@mantine/core";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronLeft, ChevronRight } from "lucide-react";
import {
	type FormEvent,
	useCallback,
	useEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import { formatErrorMessage } from "../../lib/formatError";
import { frontendLog } from "../../lib/frontendLog";
import {
	useLicenseState,
	useRequestLicensePasswordReset,
	useSettings,
	useSignUpLicense,
	useStartLicenseLogin,
} from "../../lib/queries";
import { type HotkeyConfig, tauriAPI } from "../../lib/tauri";
import { Logo } from "../Logo";
import {
	buildSettingsGuideAccountViewModel,
	buildSettingsGuideGroqStepViewModel,
	buildSettingsGuideWrapupViewModel,
	SETTINGS_GUIDE_STEPS,
	type SettingsGuideStep,
} from "./settingsGuideAccount";

type Phase = "welcome" | "guide";

type Step = SettingsGuideStep;

type NavStep = "welcome" | Step;

type AccountAuthMode = "sign_up" | "sign_in";

const GUIDE_STEPS: Step[] = [...SETTINGS_GUIDE_STEPS];
const NAV_STEPS: NavStep[] = ["welcome", ...GUIDE_STEPS];

function HotkeyCombo({ config }: { config: HotkeyConfig | null }) {
	const parts = useMemo(() => {
		if (!config) return null;
		const mods = config.modifiers.map(
			(m) => m.charAt(0).toUpperCase() + m.slice(1),
		);
		return [...mods, config.key];
	}, [config]);

	const comboKey = useMemo(() => (parts ? parts.join("+") : ""), [parts]);

	if (!parts) {
		return <Kbd className="hotkey-placeholder">Unassigned</Kbd>;
	}

	return (
		<span className="tang-guide-kbd-combo">
			{parts.map((part, idx) => (
				<span key={`${comboKey}-${part}`}>
					<Kbd>{part}</Kbd>
					{idx < parts.length - 1 && <span className="kbd-plus">+</span>}
				</span>
			))}
		</span>
	);
}

export function SettingsGuideOverlay({
	opened,
	onSkip,
	onFinished,
	onGoHome,
}: {
	opened: boolean;
	onSkip: () => void;
	onFinished: () => void;
	onGoHome: () => void;
}) {
	const queryClient = useQueryClient();

	const { data: settings } = useSettings();
	const licenseState = useLicenseState();
	const startLicenseLogin = useStartLicenseLogin();
	const signUpLicense = useSignUpLicense();
	const requestPasswordReset = useRequestLicensePasswordReset();
	const toggleHotkey = settings?.toggle_hotkey ?? null;
	const accountView = buildSettingsGuideAccountViewModel(licenseState.data);
	const groqView = buildSettingsGuideGroqStepViewModel(accountView);
	const wrapupView = buildSettingsGuideWrapupViewModel(accountView);
	const accountTierLabel =
		accountView.mode === "pro"
			? "Pro"
			: accountView.mode === "enterprise"
				? "Managed"
				: "Community";

	const recommendedToggleKeyLabel =
		typeof navigator !== "undefined" && /windows/i.test(navigator.userAgent)
			? "Right Alt"
			: "F3";

	const welcomeTimersRef = useRef<number[]>([]);

	const [phase, setPhase] = useState<Phase>("welcome");
	const [step, setStep] = useState<Step>("account");

	const [welcomeIconVisible, setWelcomeIconVisible] = useState(false);
	const [welcomeTextVisible, setWelcomeTextVisible] = useState(false);
	const [welcomeFadingOut, setWelcomeFadingOut] = useState(false);
	const [welcomeContinueVisible, setWelcomeContinueVisible] = useState(false);
	const [welcomeContinueSeen, setWelcomeContinueSeen] = useState(false);

	const [skipVisible, setSkipVisible] = useState(false);
	const [showInlineSignIn, setShowInlineSignIn] = useState(false);
	const [accountAuthMode, setAccountAuthMode] =
		useState<AccountAuthMode>("sign_up");
	const [accountAuthSubmittingMode, setAccountAuthSubmittingMode] =
		useState<AccountAuthMode | null>(null);
	const accountAuthPending = accountAuthSubmittingMode !== null;
	const [accountEmail, setAccountEmail] = useState("");
	const [accountPassword, setAccountPassword] = useState("");
	const [accountMessage, setAccountMessage] = useState<string | null>(null);
	const [accountError, setAccountError] = useState<string | null>(null);

	const { data: groqApiKeyValue } = useQuery({
		queryKey: ["apiKeyValue", "groq_api_key"],
		enabled: opened,
		queryFn: () => tauriAPI.getApiKey("groq_api_key"),
		staleTime: 0,
	});

	const [groqKeyValue, setGroqKeyValue] = useState("");
	const [isSavingGroqKey, setIsSavingGroqKey] = useState(false);

	const trimmedGroqKeyValue = groqKeyValue.trim();
	const savedGroqKeyValue = (groqApiKeyValue ?? "").trim();
	const isGroqKeyUnchanged =
		savedGroqKeyValue.length > 0 && trimmedGroqKeyValue === savedGroqKeyValue;

	const hasHydratedGroqKeyRef = useRef(false);

	const [finishVisible, setFinishVisible] = useState(false);
	const [finishSeen, setFinishSeen] = useState(false);

	const [dictationText, setDictationText] = useState("");
	const dictationInputRef = useRef<HTMLTextAreaElement | null>(null);

	const sampleText =
		"I can dictate with my voice anywhere, and tune settings per app and website.";

	const clearWelcomeTimers = useCallback(() => {
		for (const t of welcomeTimersRef.current) window.clearTimeout(t);
		welcomeTimersRef.current = [];
	}, []);

	const enterGuideAt = (nextStep: Step) => {
		frontendLog.info("setup-guide", `enterGuide step=${nextStep}`);
		clearWelcomeTimers();
		setPhase("guide");
		setStep(nextStep);
		setSkipVisible(true);
	};

	// Use a ref so the opened-effect can call the latest version without
	// re-triggering when `welcomeContinueSeen` changes (which would restart
	// the welcome animation in an infinite loop).
	const welcomeContinueSeenRef = useRef(false);
	welcomeContinueSeenRef.current = welcomeContinueSeen;

	const restartWelcomeSequence = useCallback(() => {
		frontendLog.info(
			"setup-guide",
			`restartWelcomeSequence (continueSeen=${welcomeContinueSeenRef.current})`,
		);
		clearWelcomeTimers();

		// Always restart the intro from scratch.
		setPhase("welcome");
		setStep("account");
		setSkipVisible(false);

		setWelcomeIconVisible(false);
		setWelcomeTextVisible(false);
		setWelcomeFadingOut(false);
		setWelcomeContinueVisible(welcomeContinueSeenRef.current);

		const timers: Array<number> = [];
		timers.push(window.setTimeout(() => setWelcomeIconVisible(true), 150));
		timers.push(window.setTimeout(() => setWelcomeTextVisible(true), 650));
		// Reveal the Continue button after the title/subtext have faded in,
		// then held on-screen briefly.
		if (!welcomeContinueSeenRef.current) {
			timers.push(
				window.setTimeout(() => {
					setWelcomeContinueSeen(true);
					setWelcomeContinueVisible(true);
				}, 1465),
			);
		}

		welcomeTimersRef.current = timers;
	}, [clearWelcomeTimers]);

	useEffect(() => {
		if (!opened) return;

		frontendLog.info("setup-guide", "overlay opened, initializing");
		queryClient.invalidateQueries({ queryKey: ["settings"] });

		// Reset guide state on open.
		restartWelcomeSequence();

		setGroqKeyValue("");
		hasHydratedGroqKeyRef.current = false;

		setFinishVisible(false);
		setFinishSeen(false);
		setWelcomeContinueSeen(false);
		setDictationText("");
		setShowInlineSignIn(false);
		setAccountAuthMode("sign_up");
		setAccountAuthSubmittingMode(null);
		setAccountEmail("");
		setAccountPassword("");
		setAccountMessage(null);
		setAccountError(null);
		return () => {
			clearWelcomeTimers();
		};
	}, [opened, queryClient, restartWelcomeSequence, clearWelcomeTimers]);

	useEffect(() => {
		if (!opened) return;
		if (hasHydratedGroqKeyRef.current) return;
		if (!groqApiKeyValue) return;

		// If a key already exists, show it in the PasswordInput (hidden by default).
		setGroqKeyValue(groqApiKeyValue);
		hasHydratedGroqKeyRef.current = true;
	}, [opened, groqApiKeyValue]);

	useEffect(() => {
		if (!opened) return;

		const onKeyDown = (e: KeyboardEvent) => {
			if (e.key !== "Escape") return;
			e.preventDefault();
			e.stopPropagation();
			// Escape exits the setup guide entirely.
			onSkip();
		};

		window.addEventListener("keydown", onKeyDown);
		return () => window.removeEventListener("keydown", onKeyDown);
	}, [opened, onSkip]);

	useEffect(() => {
		if (!opened) return;
		if (phase !== "guide") return;
		if (step !== "dictation") return;

		// Give the user a target to dictate into.
		dictationInputRef.current?.focus();
	}, [opened, phase, step]);

	useEffect(() => {
		if (!opened) return;
		if (step !== "wrapup") {
			setFinishVisible(false);
			return;
		}

		if (finishSeen) {
			setFinishVisible(true);
			return;
		}

		// Match the welcome slide timing: wait for content to be fully faded in (~280ms),
		// then hold briefly before revealing the action.
		const t = window.setTimeout(() => {
			setFinishSeen(true);
			setFinishVisible(true);
		}, 1140);
		return () => window.clearTimeout(t);
	}, [opened, step, finishSeen]);

	const nextStep = () => {
		// Bottom-right action advances to the next page.
		goForward();
	};

	const navStep: NavStep = phase === "welcome" ? "welcome" : step;
	const navIndex = NAV_STEPS.indexOf(navStep);

	const canGoBack = navIndex > 0;
	const canGoForward = (() => {
		if (navIndex < 0) return false;
		if (navIndex >= NAV_STEPS.length - 1) return false;

		// From the welcome slide, always allow moving forward.
		if (navStep === "welcome") return true;

		return true;
	})();
	const accountAuthFormVisible =
		phase === "guide" &&
		step === "account" &&
		!accountView.isSignedIn &&
		showInlineSignIn;

	const goBack = () => {
		if (!canGoBack) return;

		const next = NAV_STEPS[navIndex - 1];
		if (!next) return;

		if (next === "welcome") {
			restartWelcomeSequence();
			return;
		}

		enterGuideAt(next);
	};

	const handleBack = () => {
		if (accountAuthFormVisible) {
			setShowInlineSignIn(false);
			setAccountError(null);
			setAccountMessage(null);
			return;
		}

		goBack();
	};

	const goForward = () => {
		if (!canGoForward) return;

		const next = NAV_STEPS[navIndex + 1];
		if (!next) return;

		if (next === "welcome") {
			restartWelcomeSequence();
			return;
		}

		enterGuideAt(next);
	};

	const handleSaveGroqKey = async () => {
		const trimmed = groqKeyValue.trim();
		if (!trimmed) return;

		setIsSavingGroqKey(true);
		try {
			await tauriAPI.setApiKey("groq_api_key", trimmed);
			await queryClient.invalidateQueries({
				queryKey: ["apiKey", "groq_api_key"],
			});
			await queryClient.invalidateQueries({
				queryKey: ["apiKeyValue", "groq_api_key"],
			});
			await queryClient.invalidateQueries({ queryKey: ["availableProviders"] });

			// Keep the value in-state so if the user navigates back in this same
			// guide session, the field stays prefilled.
			setGroqKeyValue(trimmed);
			hasHydratedGroqKeyRef.current = true;
			setStep("dictation");
		} catch (err) {
			console.error("Failed to save Groq key", err);
		} finally {
			setIsSavingGroqKey(false);
		}
	};

	const showAccountForm = (mode: AccountAuthMode) => {
		setAccountAuthMode(mode);
		setAccountAuthSubmittingMode(null);
		setShowInlineSignIn(true);
		setAccountMessage(null);
		setAccountError(null);
	};

	const handleInlineAccountAuth = (event: FormEvent<HTMLFormElement>) => {
		event.preventDefault();
		setAccountAuthSubmittingMode(accountAuthMode);
		setAccountError(null);

		if (accountAuthMode === "sign_up") {
			setAccountMessage("Creating your free Kolboo account…");
			signUpLicense.mutate(
				{
					email: accountEmail,
					password: accountPassword,
				},
				{
					onSuccess: (response) => {
						setAccountPassword("");
						if (response.confirmation_required) {
							setAccountAuthMode("sign_in");
							setAccountMessage(
								"Account created. Check your email to confirm it, then return here to sign in.",
							);
							return;
						}

						const model = buildSettingsGuideAccountViewModel(response.state);
						setAccountMessage(
							model.hasPaidAccess
								? "Account created and signed in. Pro-only features will be available where enabled. If a new invite or upgrade still looks missing, you can refresh it later from Account."
								: "Account created and signed in. Payment is optional, so Kolboo will continue in Community/BYOK mode. If a new invite or upgrade still looks missing, you can refresh it later from Account.",
						);
					},
					onError: (error) => {
						setAccountMessage(null);
						setAccountError(formatErrorMessage(error));
					},
					onSettled: () => setAccountAuthSubmittingMode(null),
				},
			);
			return;
		}

		setAccountMessage("Signing in…");
		startLicenseLogin.mutate(
			{
				provider_hint: "personal",
				email: accountEmail,
				password: accountPassword,
			},
			{
				onSuccess: (state) => {
					const model = buildSettingsGuideAccountViewModel(state);
					setAccountPassword("");
					setAccountMessage(
						model.hasPaidAccess
							? "Signed in. Pro-only features will be available where enabled. If a new invite or upgrade still looks missing, you can refresh it later from Account."
							: "Signed in. Payment is optional, so Kolboo will continue in Community/BYOK mode. If a new invite or upgrade still looks missing, you can refresh it later from Account.",
					);
				},
				onError: (error) => {
					setAccountMessage(null);
					setAccountError(formatErrorMessage(error));
				},
				onSettled: () => setAccountAuthSubmittingMode(null),
			},
		);
	};

	const handlePasswordReset = () => {
		const email = accountEmail.trim();
		setAccountError(null);
		setAccountMessage(null);
		if (!email) {
			setAccountError("Enter your email address first.");
			return;
		}

		requestPasswordReset.mutate(email, {
			onSuccess: () => {
				setAccountMessage(
					"Check your email for a link to choose a new password.",
				);
			},
			onError: (error) => setAccountError(formatErrorMessage(error)),
		});
	};

	const handleBrowserAccountAuth = () => {
		setAccountError(null);
		setAccountMessage("Opening Kolboo account sign-in in your browser…");
		startLicenseLogin.mutate(
			{ provider_hint: "personal" },
			{
				onSuccess: (state) => {
					const model = buildSettingsGuideAccountViewModel(state);
					setAccountMessage(
						model.hasPaidAccess
							? "Signed in. Your managed features are ready where enabled."
							: "Signed in. Kolboo will continue in Community/BYOK mode.",
					);
				},
				onError: (error) => {
					setAccountMessage(null);
					setAccountError(formatErrorMessage(error));
				},
			},
		);
	};

	if (!opened) return null;

	return (
		<div className="tang-guide-overlay" role="dialog" aria-modal="true">
			{phase === "welcome" && (
				<div
					className={
						"tang-guide-welcome" +
						(welcomeFadingOut ? " tang-guide-welcome--fade-out" : "")
					}
				>
					<div className="tang-guide-welcome-center">
						<div
							className={
								"tang-guide-welcome-logo" +
								(welcomeIconVisible
									? " tang-guide-fade-in tang-guide-fade-in-slow"
									: "")
							}
						>
							<Logo size={140} />
						</div>
						<div
							className={
								"tang-guide-welcome-text" +
								(welcomeTextVisible
									? " tang-guide-fade-in tang-guide-fade-in-slow"
									: "")
							}
						>
							<Title order={2} style={{ marginTop: 18 }}>
								Welcome to Kolboo
							</Title>
							<Text c="dimmed" size="sm" style={{ marginTop: 6 }}>
								Let’s set up your account options and voice dictation.
							</Text>
						</div>
					</div>

					<button
						type="button"
						className={
							"tang-guide-continue" +
							(welcomeContinueVisible || welcomeContinueSeen
								? " tang-guide-fade-in"
								: "")
						}
						onClick={() => enterGuideAt("account")}
					>
						<span>Start</span>
						<ChevronRight size={16} />
					</button>
				</div>
			)}

			{phase === "guide" && (
				<>
					{skipVisible &&
						navIndex < NAV_STEPS.length - 1 &&
						step !== "account" && (
							<button
								type="button"
								className="tang-guide-skip tang-guide-fade-in"
								onClick={nextStep}
							>
								<span>Next</span>
								<ChevronRight size={16} />
							</button>
						)}

					{canGoBack && (
						<button
							type="button"
							className="tang-guide-back tang-guide-fade-in"
							onClick={handleBack}
							disabled={accountAuthPending}
						>
							<ChevronLeft size={16} />
							<span>Back</span>
						</button>
					)}

					<div className="tang-guide-content tang-guide-fade-in">
						{step === "account" && (
							<div className="tang-guide-step">
								<Title order={3}>
									{accountView.isSignedIn ? "Account setup" : accountView.title}
								</Title>
								{!accountAuthFormVisible ? (
									<Text
										className="tang-guide-account-intro"
										c="dimmed"
										size="sm"
									>
										{accountView.description}
									</Text>
								) : null}

								{accountView.isSignedIn ? (
									<Group justify="center" gap="xs" mt="md">
										<Text size="sm" c="dimmed">
											{accountView.detail}
										</Text>
										<Badge color="green" variant="light" size="sm">
											{accountTierLabel}
										</Badge>
									</Group>
								) : null}

								{accountMessage && !accountView.isSignedIn ? (
									<Text size="sm" className="tang-guide-account-message">
										{accountMessage}
									</Text>
								) : null}
								{accountError ? (
									<Text size="sm" className="tang-guide-account-error">
										{accountError}
									</Text>
								) : null}

								{accountView.isSignedIn ? (
									<Group justify="center" mt="lg">
										<Button color="orange" onClick={() => enterGuideAt("groq")}>
											Continue setup
										</Button>
									</Group>
								) : (
									<div
										className={`tang-guide-account-choice${
											accountAuthFormVisible
												? " tang-guide-account-choice--form"
												: ""
										}`}
									>
										{!showInlineSignIn ? (
											<div className="tang-guide-account-actions">
												<Button
													type="button"
													color="orange"
													onClick={() => showAccountForm("sign_up")}
												>
													Create account
												</Button>
												<Button
													type="button"
													variant="default"
													onClick={() => showAccountForm("sign_in")}
												>
													Sign in
												</Button>
												<Button
													type="button"
													variant="subtle"
													onClick={() => enterGuideAt("groq")}
												>
													Continue without an account
												</Button>
											</div>
										) : (
											<form
												className="tang-guide-account-form"
												onSubmit={handleInlineAccountAuth}
											>
												<div className="tang-guide-account-form-heading">
													<Text fw={700} size="lg">
														{accountAuthMode === "sign_up"
															? "Create account"
															: "Sign in"}
													</Text>
												</div>
												<TextInput
													label="Email"
													type="email"
													value={accountEmail}
													onChange={(event) =>
														setAccountEmail(event.currentTarget.value)
													}
													autoComplete="email"
													disabled={accountAuthPending}
													required
												/>
												<PasswordInput
													label="Password"
													value={accountPassword}
													onChange={(event) =>
														setAccountPassword(event.currentTarget.value)
													}
													autoComplete={
														accountAuthMode === "sign_up"
															? "new-password"
															: "current-password"
													}
													disabled={accountAuthPending}
													required
												/>
												{accountAuthMode === "sign_in" ? (
													<Group justify="space-between">
														<Button
															type="button"
															variant="subtle"
															onClick={handlePasswordReset}
															loading={requestPasswordReset.isPending}
															disabled={accountAuthPending}
															style={{ paddingInline: 0 }}
														>
															Forgot password?
														</Button>
														<Button
															type="button"
															variant="subtle"
															onClick={handleBrowserAccountAuth}
															loading={
																startLicenseLogin.isPending &&
																accountAuthSubmittingMode === null
															}
															disabled={
																accountAuthPending ||
																requestPasswordReset.isPending
															}
															style={{ paddingInline: 0 }}
														>
															Use browser instead
														</Button>
													</Group>
												) : (
													<Button
														type="button"
														variant="subtle"
														onClick={handleBrowserAccountAuth}
														loading={startLicenseLogin.isPending}
														disabled={accountAuthPending}
														style={{
															alignSelf: "flex-start",
															paddingInline: 0,
														}}
													>
														Use browser instead
													</Button>
												)}
												<Group justify="flex-end">
													<Button
														type="submit"
														loading={
															accountAuthSubmittingMode === accountAuthMode
														}
													>
														{accountAuthMode === "sign_up"
															? "Create free account"
															: "Sign in"}
													</Button>
												</Group>
											</form>
										)}
									</div>
								)}
							</div>
						)}

						{step === "groq" && (
							<div className="tang-guide-step">
								<Title order={3}>{groqView.title}</Title>
								<Text c="dimmed" size="sm" style={{ marginTop: 8 }}>
									{groqView.description}{" "}
									<Anchor
										href="https://console.groq.com/keys"
										target="_blank"
										rel="noreferrer"
									>
										https://console.groq.com/keys
									</Anchor>
								</Text>
								{groqView.helper ? (
									<Text c="dimmed" size="xs" style={{ marginTop: 10 }}>
										{groqView.helper}
									</Text>
								) : null}

								<div style={{ marginTop: 18 }}>
									<div style={{ marginTop: 12 }}>
										<PasswordInput
											value={groqKeyValue}
											onChange={(e) => setGroqKeyValue(e.currentTarget.value)}
											placeholder="Paste your Groq API key"
											autoFocus
											styles={{
												input: {
													backgroundColor: "var(--bg-elevated)",
													borderColor: "var(--border-default)",
													color: "var(--text-primary)",
												},
											}}
											onKeyDown={(e) => {
												if (e.key === "Enter") void handleSaveGroqKey();
											}}
										/>
										<Group justify="flex-end" mt="sm">
											<Button
												color="orange"
												onClick={handleSaveGroqKey}
												loading={isSavingGroqKey}
												disabled={
													!trimmedGroqKeyValue ||
													isSavingGroqKey ||
													isGroqKeyUnchanged
												}
											>
												{groqView.submitLabel}
											</Button>
										</Group>
									</div>
								</div>
							</div>
						)}

						{step === "dictation" && (
							<div className="tang-guide-step">
								<Title order={3}>Voice dictation test</Title>
								<Text c="dimmed" size="sm" style={{ marginTop: 8 }}>
									{!settings ? (
										<>Loading your shortcut…</>
									) : toggleHotkey ? (
										<>
											Use your toggle recording shortcut{" "}
											<HotkeyCombo config={toggleHotkey} />. Press once to start
											recording, then press again to stop.
										</>
									) : (
										<>
											Your toggle recording shortcut is{" "}
											<HotkeyCombo config={null} />. Set one in Settings →
											Hotkeys (recommended:{" "}
											<Kbd>{recommendedToggleKeyLabel}</Kbd>), then press it
											once to start recording and again to stop.
										</>
									)}
								</Text>

								<div className="tang-guide-copy" style={{ marginTop: 14 }}>
									<Text size="sm" style={{ marginBottom: 6, opacity: 0.9 }}>
										Say something like:
									</Text>
									<div className="tang-guide-copy-box">
										<Text size="sm">{sampleText}</Text>
									</div>
								</div>

								<div style={{ marginTop: 14 }}>
									<Textarea
										ref={dictationInputRef}
										value={dictationText}
										onChange={(e) => setDictationText(e.currentTarget.value)}
										placeholder="Dictate here…"
										minRows={4}
										autosize
										styles={{
											input: {
												backgroundColor: "var(--bg-elevated)",
												borderColor: "var(--border-default)",
												color: "var(--text-primary)",
											},
										}}
									/>
								</div>
							</div>
						)}

						{step === "wrapup" && (
							<div className="tang-guide-step">
								<Title order={3}>{wrapupView.title}</Title>
								<Text c="dimmed" size="sm" style={{ marginTop: 8 }}>
									{wrapupView.description}
								</Text>
								<Text c="dimmed" size="sm" style={{ marginTop: 12 }}>
									{wrapupView.detail}
								</Text>
							</div>
						)}
					</div>

					{step === "wrapup" && (
						<button
							type="button"
							className={
								"tang-guide-finish" +
								(finishSeen || finishVisible ? " tang-guide-fade-in" : "")
							}
							onClick={() => {
								onFinished();
								onGoHome();
							}}
							disabled={!finishSeen && !finishVisible}
						>
							Finish
						</button>
					)}
				</>
			)}
		</div>
	);
}
