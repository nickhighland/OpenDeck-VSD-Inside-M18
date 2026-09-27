import { writable } from "svelte/store";

export type ToastKind = "info" | "success" | "error";
export type Toast = { id: number; kind: ToastKind; title: string; detail?: string };

export const toasts = writable<Toast[]>([]);

let nextId = 1;

export function dismissToast(id: number) {
	toasts.update((list) => list.filter((toast) => toast.id !== id));
}

/** Show a short notification. Errors stay longer so they can be read. */
export function toast(kind: ToastKind, title: string, detail?: string) {
	const id = nextId++;
	toasts.update((list) => [...list.slice(-3), { id, kind, title, detail }]);
	setTimeout(() => dismissToast(id), kind === "error" ? 7000 : 3500);
}

/** Turn an invoke rejection (usually a string from the backend) into readable text. */
export function errorText(error: unknown): string {
	if (typeof error === "string") return error;
	if (error instanceof Error) return error.message;
	try {
		return JSON.stringify(error);
	} catch {
		return String(error);
	}
}

/** Run a backend call, reporting a failure as a toast instead of an unhandled rejection. */
export async function attempt<T>(title: string, action: () => Promise<T>): Promise<T | undefined> {
	try {
		return await action();
	} catch (error) {
		toast("error", title, errorText(error));
		return undefined;
	}
}
