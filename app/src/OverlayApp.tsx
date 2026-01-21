import { Loader } from "@mantine/core";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import RecordingControl from "./overlay/RecordingControl";
import "./app.css";

export default function OverlayApp() {
	const [ready, setReady] = useState(false);

	// Sync pipeline config on mount
	useEffect(() => {
		const init = async () => {
			try {
				await invoke("sync_pipeline_config");
				setReady(true);
			} catch (error) {
				console.error("[Overlay] Failed to sync pipeline config:", error);
				// Still show UI even if sync fails
				setReady(true);
			}
		};

		void init();
	}, []);

	if (!ready) {
		return (
			<div
				className="flex items-center justify-center"
				style={{
					width: 56,
					height: 56,
					backgroundColor: "rgba(0, 0, 0, 0.9)",
					borderRadius: 16,
				}}
			>
				<Loader size="xs" color="orange" />
			</div>
		);
	}

	return <RecordingControl />;
}
