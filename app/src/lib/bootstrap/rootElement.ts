export function getRootElementOrThrow(id = "root"): HTMLElement {
	const rootElement = document.getElementById(id);
	if (!rootElement) {
		throw new Error("Root element not found");
	}
	return rootElement;
}
