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
	const [formMode, setFormMode] = useState<"sign_up" | "sign_in">("sign_in");
	const showSignIn = !signedIn || reauthRequired;
	const creating = !signedIn && formMode === "sign_up";
	const formPending = loginPending || signupPending;
	const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
		event.preventDefault();
		if (formPending) return;
		if (creating) onPasswordSignUp(email.trim(), password);
		else onPasswordSignIn(email.trim(), password);
	};
	return (
		<Card withBorder radius="lg" className="account-panel account-actions">
			<Stack gap="lg">
				<Title order={2} size="h3">
					{showSignIn
						? creating
							? "Create your account"
							: "Sign in to Kolboo"
						: "Your account"}
				</Title>
				{showSignIn && (
					<Box component="form" onSubmit={handleSubmit}>
						<Stack gap="md">
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
								autoComplete={creating ? "new-password" : "current-password"}
								disabled={formPending}
								required
							/>
							{!creating && (
								<Group justify="flex-end">
									<Button
										variant="subtle"
										size="compact-xs"
										onClick={onBrowserSignIn}
										disabled={formPending}
										type="button"
									>
										Forgot password?
									</Button>
								</Group>
							)}
							<Button type="submit" fullWidth loading={formPending}>
								{creating ? "Create account" : "Sign in"}
							</Button>
							<Button
								type="button"
								variant="default"
								fullWidth
								onClick={onBrowserSignIn}
								disabled={formPending}
							>
								Continue in browser
							</Button>
							{!signedIn && (
								<Button
									type="button"
									variant="subtle"
									color="gray"
									disabled={formPending}
									onClick={() => setFormMode(creating ? "sign_in" : "sign_up")}
								>
									{creating
										? "Already have an account? Sign in"
										: "Create an account"}
								</Button>
							)}
						</Stack>
					</Box>
				)}
				{signedIn ? (
					<Group gap="sm">
						<Button
							variant="default"
							onClick={onRefresh}
							loading={refreshPending}
						>
							Refresh access
						</Button>
						{manageAvailable && (
							<Button
								variant="light"
								onClick={onManage}
								loading={managePending}
							>
								Manage account
							</Button>
						)}
						<Button
							color="gray"
							variant="subtle"
							onClick={onSignOut}
							loading={logoutPending}
						>
							Sign out
						</Button>
					</Group>
				) : (
					<Text size="xs" c="dimmed" ta="center">
						Local models and your own keys work without an account.
					</Text>
				)}
			</Stack>
		</Card>
	);
}
