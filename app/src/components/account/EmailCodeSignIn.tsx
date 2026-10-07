import { Alert, Box, Button, Group, PasswordInput, Stack, Text, TextInput } from "@mantine/core";
import { type FormEvent, useEffect, useRef, useState } from "react";
import { formatErrorMessage } from "../../lib/formatError";

export type EmailCodeSignInActions = {
    requestCode: (email: string) => Promise<unknown>;
    verifyCode: (email: string, code: string) => Promise<unknown>;
    passwordSignIn: (email: string, password: string) => Promise<unknown>;
    resetPassword: (email: string) => Promise<unknown>;
    browserSignIn: () => Promise<unknown>;
    cancel: () => Promise<unknown>;
};

export function EmailCodeSignIn({ actions }: { actions: EmailCodeSignInActions }) {
    const [mode, setMode] = useState<"email" | "code" | "password">("email");
    const [email, setEmail] = useState("");
    const [code, setCode] = useState("");
    const [password, setPassword] = useState("");
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [notice, setNotice] = useState<string | null>(null);
    const [resendAt, setResendAt] = useState(0);
    const [now, setNow] = useState(Date.now());
    const generation = useRef(0);
    const pending = useRef(false);
    const cancel = useRef(actions.cancel);
    cancel.current = actions.cancel;
    useEffect(() => () => {
        generation.current += 1;
        if (pending.current) void cancel.current().catch(() => undefined);
    }, []);
    useEffect(() => {
        if (resendAt <= now) return;
        const timer = window.setInterval(() => setNow(Date.now()), 1000);
        return () => window.clearInterval(timer);
    }, [resendAt, now]);

    async function perform(action: () => Promise<unknown>, success: () => void) {
        if (pending.current) return;
        const ticket = ++generation.current;
        pending.current = true;
        setBusy(true); setError(null); setNotice(null);
        try {
            await action();
            if (ticket === generation.current) success();
        } catch (error) {
            if (ticket === generation.current) setError(formatErrorMessage(error));
        } finally {
            if (ticket === generation.current) { pending.current = false; setBusy(false); }
        }
    }
    function sent() {
        setMode("code"); setCode("");
        setNow(Date.now()); setResendAt(Date.now() + 60_000);
    }
    function reset() {
        const ticket = ++generation.current; pending.current = false;
        setBusy(false); setMode("email"); setCode(""); setPassword(""); setError(null); setNotice(null);
        void actions.cancel().catch(error => {
            if (ticket === generation.current) setError(formatErrorMessage(error));
        });
    }
    function submit(event: FormEvent<HTMLFormElement>) {
        event.preventDefault();
        const address = email.trim();
        if (mode === "email") void perform(() => actions.requestCode(address), sent);
        else if (mode === "code") void perform(() => actions.verifyCode(address, code.trim()), () => setCode(""));
        else void perform(() => actions.passwordSignIn(address, password), () => setPassword(""));
    }
    const seconds = Math.max(0, Math.ceil((resendAt - now) / 1000));
    return (
        <Box component="form" onSubmit={submit}>
            <Stack gap="md">
                {error && <Alert color="red" role="alert">{error}</Alert>}
                {notice && <Text size="sm" role="status">{notice}</Text>}
                {mode === "code" ? <>
                    <Text size="sm" c="dimmed">Code sent to {email.trim()}</Text>
                    <TextInput label="Email code" autoComplete="one-time-code" inputMode="numeric" pattern="[0-9]{6}" maxLength={6} value={code} onChange={event => setCode(event.currentTarget.value)} required disabled={busy} autoFocus />
                </> : <TextInput label="Email" type="email" autoComplete="email" value={email} onChange={event => setEmail(event.currentTarget.value)} required disabled={busy} />}
                {mode === "password" && <>
                    <PasswordInput label="Password" autoComplete="current-password" value={password} onChange={event => setPassword(event.currentTarget.value)} required disabled={busy} />
                    <Button type="button" size="compact-sm" variant="subtle" disabled={busy || !email.trim()} onClick={() => void perform(() => actions.resetPassword(email.trim()), () => setNotice("Check your email for a password reset link."))}>Forgot password?</Button>
                </>}
                <Button type="submit" fullWidth loading={busy}>{mode === "email" ? "Continue with email" : mode === "code" ? "Verify code" : "Sign in"}</Button>
                {mode === "code" ? <Group justify="space-between">
                    <Button type="button" variant="subtle" size="compact-sm" disabled={busy || seconds > 0} onClick={() => void perform(() => actions.requestCode(email.trim()), sent)}>{seconds > 0 ? `Resend in ${seconds}s` : "Resend code"}</Button>
                    <Button type="button" variant="subtle" size="compact-sm" onClick={reset}>{busy ? "Cancel" : "Change email"}</Button>
                </Group> : <Group justify="center">
                    <Button type="button" variant="subtle" color="gray" size="compact-sm" disabled={busy} onClick={() => { setMode(mode === "password" ? "email" : "password"); setPassword(""); setError(null); setNotice(null); }}>{mode === "password" ? "Use email code" : "Use password"}</Button>
                    <Button type="button" variant="subtle" color="gray" size="compact-sm" disabled={busy} onClick={() => void perform(actions.browserSignIn, () => setPassword(""))}>Use browser</Button>
                </Group>}
                {busy && mode !== "code" && <Button type="button" variant="subtle" size="compact-sm" onClick={reset}>Cancel</Button>}
                {mode === "email" && <Text size="xs" c="dimmed" ta="center">New accounts are invite-only. Your own API keys work without an account.</Text>}
            </Stack>
        </Box>
    );
}
