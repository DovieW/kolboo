import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef } from "react";

/**
 * Hook to forward modifier-only and special key events from the WebView to the backend.
 *
 * Why this exists:
 * WebView2 (Chromium) intercepts certain key events for menu accelerator handling
 * or passes them to the OS before they reach the Windows low-level keyboard hook (WH_KEYBOARD_LL).
 * When the WebView has focus, the backend hook never sees these events.
 *
 * Keys handled:
 * - AltRight (Right Alt / AltGr): Intercepted by Chromium for menu access
 * - Win+C (Copilot): Passes through to OS Copilot when WebView focused
 * - Win+Shift+F23 (Some Copilot keyboards): Same issue
 *
 * This hook:
 * 1. Listens for keydown/keyup events for these keys
 * 2. Calls preventDefault() to stop browser/OS default behavior
 * 3. Forwards the event to the backend via a Tauri command
 *
 * Usage: Call this hook in the root component of each WebView window.
 */
export function useModifierKeyForwarder() {
	// Track held state to avoid duplicate events
	const altRightHeld = useRef(false);
	const metaHeld = useRef(false);
	const copilotChordFired = useRef(false);

	useEffect(() => {
		const handleKeyDown = (e: KeyboardEvent) => {
			// Track Meta (Win) key state
			if (e.key === "Meta") {
				metaHeld.current = true;
				copilotChordFired.current = false;
			}

			// AltRight: e.code === "AltRight", e.key === "Alt", e.location === 2 (right)
			if (e.code === "AltRight" || (e.key === "Alt" && e.location === 2)) {
				// Prevent Chromium's menu accelerator behavior
				e.preventDefault();

				// Only forward if not already held (avoid key repeat spam)
				if (!altRightHeld.current) {
					altRightHeld.current = true;
					invoke("forward_modifier_key_event", {
						key: "AltRight",
						isDown: true,
					}).catch((err: unknown) => {
						console.error("Failed to forward AltRight keydown:", err);
					});
				}
				return;
			}

			// Copilot: Win+C or Win+Shift+F23
			if (
				metaHeld.current &&
				!copilotChordFired.current &&
				(e.code === "KeyC" || (e.shiftKey && e.code === "F23"))
			) {
				e.preventDefault();
				copilotChordFired.current = true;

				// Forward as Copilot key down
				invoke("forward_modifier_key_event", {
					key: "Copilot",
					isDown: true,
				}).catch((err: unknown) => {
					console.error("Failed to forward Copilot keydown:", err);
				});
				return;
			}
		};

		const handleKeyUp = (e: KeyboardEvent) => {
			// Track Meta (Win) key state
			if (e.key === "Meta") {
				metaHeld.current = false;

				// If Copilot chord was active, send key up
				if (copilotChordFired.current) {
					copilotChordFired.current = false;
					invoke("forward_modifier_key_event", {
						key: "Copilot",
						isDown: false,
					}).catch((err: unknown) => {
						console.error("Failed to forward Copilot keyup:", err);
					});
				}
			}

			// AltRight
			if (e.code === "AltRight" || (e.key === "Alt" && e.location === 2)) {
				e.preventDefault();

				if (altRightHeld.current) {
					altRightHeld.current = false;
					invoke("forward_modifier_key_event", {
						key: "AltRight",
						isDown: false,
					}).catch((err: unknown) => {
						console.error("Failed to forward AltRight keyup:", err);
					});
				}
			}
		};

		// Use capture phase to intercept before any other handlers
		window.addEventListener("keydown", handleKeyDown, { capture: true });
		window.addEventListener("keyup", handleKeyUp, { capture: true });

		return () => {
			window.removeEventListener("keydown", handleKeyDown, { capture: true });
			window.removeEventListener("keyup", handleKeyUp, { capture: true });
		};
	}, []);
}
