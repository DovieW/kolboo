import {
	Box,
	Button,
	Card,
	Group,
	PasswordInput,
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
	refreshPending: boolean;
	logoutPending: boolean;
	managePending: boolean;
	onPasswordSignIn: (email: string, password: string) => void;
	onBrowserSignIn: () => void;
	onRefresh: () => void;
	onManage: () => void;
	onSignOut: () => void;
}) {
	const {
		signedIn,
		reauthRequired,
		loginPending,
		refreshPending,
		logoutPending,
		managePending,
		onPasswordSignIn,
		onBrowserSignIn,
		onRefresh,
		onManage,
		onSignOut,
	} = props;
	const [email, setEmail] = useState("");
	const [password, setPassword] = useState("");

	const showSignIn = !signedIn || reauthRequired;
	const actionTitle = signedIn ? "Manage your session" : "Sign in to Kolboo";
	const introCopy = signedIn
		? "Refresh your managed access, open account management, or sign out."
		: "Use an existing Supabase account to connect Personal or Managed Business access.";
	const browserCopy = reauthRequired
		? "Kolboo will reopen your browser so you can restore managed access without entering credentials in-app."
		: "OAuth sign-in opens your browser and finishes the desktop session after the callback returns.";
	const signInLabel =
		reauthRequired && signedIn ? "Re-authenticate" : "Sign in";

	const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
		event.preventDefault();
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
							<Text c="dimmed" size="sm">
								Sign in with the email and password for a Supabase user. This
								works with users created manually in the Supabase dashboard.
							</Text>
							<TextInput
								label="Email"
								type="email"
								value={email}
								onChange={(event) => setEmail(event.currentTarget.value)}
								autoComplete="email"
								disabled={loginPending}
								required
							/>
							<PasswordInput
								label="Password"
								value={password}
								onChange={(event) => setPassword(event.currentTarget.value)}
								autoComplete="current-password"
								disabled={loginPending}
								required
							/>

							<Group gap="sm" wrap="wrap">
								<Button type="submit" loading={loginPending}>
									{signInLabel}
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
								>
									Use OAuth provider instead
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
							Sign in to unlock managed Personal or Managed Business access.
						</Text>
					)}
				</Group>
			</Stack>
		</Card>
	);
}
