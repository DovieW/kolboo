import type { UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";

export const AUDIO_FILE_EXTENSIONS = [
	"wav",
	"wave",
	"mp3",
	"flac",
	"aac",
];

export function audioFileName(path: string): string {
	return path.split(/[\\/]/).at(-1) || "Audio file";
}

export function isSupportedAudioFile(path: string): boolean {
	return AUDIO_FILE_EXTENSIONS.includes(
		path.split(".").at(-1)?.toLowerCase() ?? "",
	);
}

export const audioFilesAPI = {
	choose: async (): Promise<string | null> => {
		const selected = await open({
			multiple: false,
			directory: false,
			title: "Choose audio to transcribe",
			filters: [{ name: "Audio", extensions: AUDIO_FILE_EXTENSIONS }],
		});
		return typeof selected === "string" ? selected : null;
	},
	listenForDrop: (
		onDrop: (paths: string[]) => void,
		onHover: (hovered: boolean) => void,
	): Promise<UnlistenFn> =>
		getCurrentWebview().onDragDropEvent(({ payload }) => {
			onHover(payload.type === "enter" || payload.type === "over");
			if (payload.type === "drop") onDrop(payload.paths);
		}),
};
