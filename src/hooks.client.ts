// In development, opening the editor in a normal browser (outside Tauri)
// starts a preview with simulated data instead of failing on every backend
// call. Production builds drop this branch entirely.
if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
	const { installPreviewMocks } = await import("$lib/devPreview");
	installPreviewMocks();
}

export {};
