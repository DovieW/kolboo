declare module "@tauri-apps/plugin-dialog" {
	export type DialogFilter = {
		name: string;
		extensions: string[];
	};

	export interface OpenDialogOptions {
		title?: string;
		multiple?: boolean;
		directory?: boolean;
		recursive?: boolean;
		filters?: DialogFilter[];
		defaultPath?: string;
	}

	export function open(
		options?: OpenDialogOptions,
	): Promise<string | string[] | null>;

	export interface SaveDialogOptions {
		title?: string;
		filters?: DialogFilter[];
		defaultPath?: string;
	}

	export function save(options?: SaveDialogOptions): Promise<string | null>;
}
