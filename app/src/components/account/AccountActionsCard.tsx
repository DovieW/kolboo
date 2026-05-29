import {
	Box,
	Button,
	Card,
	Group,
	PasswordInput,
	SegmentedControl,
	Stack,
	Text,
	TextInput,
	Title,
} from "@mantine/core";
import { type FormEvent, useState } from "react";

export function AccountActionsCard(props: {
	signedIn: boolean;
	reauthRequired: boolean;
	loginPending: boolean;
	signupPending: boolean;
	refreshPending: boolean;
	logoutPending: boolean;
	managePending: boolean;
	manageAvailable: boolean;
	onPasswordSignIn: (email: string, password: string) => void;
	onPasswordSignUp: (email: string, password: string) => void;
	onBrowserSignIn: () => void;
	onRefresh: () => void;
	onManage: () => void;
	onSignOut: () => void;
}) {
	const {
		signedIn,
		reauthRequired,
		loginPending,
		signupPending,
		refreshPending,
		logoutPending,
		managePending,
		manageAvailable,
		onPasswordSignIn,
		onPasswordSignUp,
		onBrowserSignIn,
		onRefresh,
		onManage,
		onSignOut,
	} = props;
	const [email, setEmail] = useState("");
	const [password, setPassword] = useState("");
	const [formMode, setFormMode] = useState<"sign_up" | "sign_in">("sign_up");

	const showSignIn = !signedIn || reauthRequired;
	const actionTitle = signedIn ? "Manage your session" : "Sign in to Kolboo";
	const introCopy = signedIn
		? "Refresh your managed access, open account management, or sign out. If organization access or billing changed recently, use Refresh access to pull the latest state."
		: "Create a free Kolboo account or sign in to save a Community/BYOK session now, then upgrade to Personal/Pro or Managed Business later for settings sync and managed inference.";
	const browserCopy = reauthRequired
		? "Kolboo will reopen your browser so you can restore managed access without re-entering everything in-app."
		: "Browser auth opens the hosted Kolboo account page, where you can sign in, create an account, or use a magic link. That page rechecks any ready organization or paid-access claims before returning to the desktop app.";
	const signInLabel =
		reauthRequired && signedIn ? "Re-authenticate" : "Sign in";
	const effectiveFormMode = signedIn ? "sign_in" : formMode;
	const formPending =
		effectiveFormMode === "sign_up" ? signupPending : loginPending;

	const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
		event.preventDefault();
		if (effectiveFormMode === "sign_up") {
			onPasswordSignUp(email, password);
			return;
		}

		onPasswordSignIn(email, password);
	};

	return (
		<Card withBorder radius="lg" className="account-panel">
			<Stack gap="md">
				<Text className="account-panel-kicker">Actions</Text>
				<Title order={3}>{actionTitle}</Title>
				<Text c="dimmed" size="sm">
					{introCopy}
				</Text>

				{showSignIn ? (
					<Box component="form" onSubmit={handleSubmit}>
						<Stack gap="sm">
							{!signedIn ? (
								<SegmentedControl
									value={formMode}
									onChange={(value) =>
										setFormMode(value as "sign_up" | "sign_in")
									}
									data={[
										{ value: "sign_up", label: "Create account" },
										{ value: "sign_in", label: "Sign in" },
									]}
									disabled={loginPending || signupPending}
								/>
							) : null}
							<Text c="dimmed" size="sm">
								{effectiveFormMode === "sign_up"
									? "Create a free self-serve account. If email confirmation is required, confirm the email first, then come back here or use browser auth to finish sign-in."
									: "Sign in with the email and password for your account. This works for self-serve accounts you created yourself or accounts an operator created for you."}
							</Text>
							<TextInput
								label="Email"
								type="email"
								value={email}
								onChange={(event) => setEmail(event.currentTarget.value)}
								autoComplete="email"
								disabled={formPending}
								required
							/>
							<PasswordInput
								label="Password"
								value={password}
								onChange={(event) => setPassword(event.currentTarget.value)}
								autoComplete={
									effectiveFormMode === "sign_up"
										? "new-password"
										: "current-password"
								}
								disabled={formPending}
								required
							/>

							<Group gap="sm" wrap="wrap">
								<Button type="submit" loading={formPending}>
									{effectiveFormMode === "sign_up"
										? "Create free account"
										: signInLabel}
								</Button>
								{signedIn ? (
									<Button
										type="button"
										variant="default"
										onClick={onSignOut}
										loading={logoutPending}
									>
										Sign out
									</Button>
								) : null}
							</Group>

							<Text c="dimmed" size="sm">
								{browserCopy}
							</Text>

							<Group gap="sm" wrap="wrap">
								<Button
									type="button"
									variant="subtle"
									onClick={onBrowserSignIn}
									loading={loginPending}
									disabled={signupPending}
								>
									Use browser auth instead
								</Button>
							</Group>
						</Stack>
					</Box>
				) : null}

				<Group gap="sm" wrap="wrap">
					{signedIn ? (
						<>
							<Button
								variant="default"
								onClick={onRefresh}
								loading={refreshPending}
							>
								Refresh access
							</Button>
							<Button
								variant="light"
								onClick={onManage}
								loading={managePending}
								disabled={!manageAvailable}
							>
								Manage account
							</Button>
							<Button
								color="red"
								variant="subtle"
								onClick={onSignOut}
								loading={logoutPending}
							>
								Sign out
							</Button>
						</>
					) : (
						<Text c="dimmed" size="sm">
							Sign in to save a Community/BYOK session now, then upgrade to
							Personal/Pro or Managed Business later for settings sync and
							managed inference.
						</Text>
					)}
				</Group>

				{signedIn && !manageAvailable ? (
					<Text c="dimmed" size="sm">
						Billing portal access is intentionally deferred in this shared-dev
						pilot.
					</Text>
				) : null}
			</Stack>
		</Card>
	);
}
