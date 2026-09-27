/**
 * Keyboard shortcuts for M18 hotkey actions, stored as Enigo token sequences
 * (RON), for example `[k(Meta,Press),r(40),k(Meta,Release)]` for ⌘K.
 *
 * On macOS keys are sent as virtual key codes (`r(code)`), exactly as the VSD
 * Craft importer does: they name the physical key, so a recorded shortcut
 * works whatever keyboard layout is active. Elsewhere, named Enigo keys are used.
 */

export type Modifier = "Meta" | "Control" | "Alt" | "Shift";
export const MODIFIERS: Modifier[] = ["Control", "Alt", "Shift", "Meta"];

export const isMac = typeof navigator !== "undefined" && /Mac/i.test(navigator.userAgent);

export const MODIFIER_SYMBOLS: Record<Modifier, string> = isMac
	? { Control: "⌃", Alt: "⌥", Shift: "⇧", Meta: "⌘" }
	: { Control: "Ctrl", Alt: "Alt", Shift: "Shift", Meta: "Win" };

export const MODIFIER_NAMES: Record<Modifier, string> = isMac
	? { Control: "Control", Alt: "Option", Shift: "Shift", Meta: "Command" }
	: { Control: "Ctrl", Alt: "Alt", Shift: "Shift", Meta: "Windows" };

type KeyDefinition = { code: string; label: string; mac: number; enigo: string };

// Apple's HIToolbox virtual key codes (Events.h), keyed by KeyboardEvent.code.
const KEY_TABLE: KeyDefinition[] = [
	...[
		["KeyA", 0x00],
		["KeyS", 0x01],
		["KeyD", 0x02],
		["KeyF", 0x03],
		["KeyH", 0x04],
		["KeyG", 0x05],
		["KeyZ", 0x06],
		["KeyX", 0x07],
		["KeyC", 0x08],
		["KeyV", 0x09],
		["KeyB", 0x0b],
		["KeyQ", 0x0c],
		["KeyW", 0x0d],
		["KeyE", 0x0e],
		["KeyR", 0x0f],
		["KeyY", 0x10],
		["KeyT", 0x11],
		["KeyO", 0x1f],
		["KeyU", 0x20],
		["KeyI", 0x22],
		["KeyP", 0x23],
		["KeyL", 0x25],
		["KeyJ", 0x26],
		["KeyK", 0x28],
		["KeyN", 0x2d],
		["KeyM", 0x2e],
	].map(([code, mac]) => ({ code: code as string, label: (code as string).slice(3), mac: mac as number, enigo: `Unicode('${(code as string).slice(3).toLowerCase()}')` })),
	...[
		["Digit1", 0x12],
		["Digit2", 0x13],
		["Digit3", 0x14],
		["Digit4", 0x15],
		["Digit5", 0x17],
		["Digit6", 0x16],
		["Digit7", 0x1a],
		["Digit8", 0x1c],
		["Digit9", 0x19],
		["Digit0", 0x1d],
	].map(([code, mac]) => ({ code: code as string, label: (code as string).slice(5), mac: mac as number, enigo: `Unicode('${(code as string).slice(5)}')` })),
	{ code: "Minus", label: "-", mac: 0x1b, enigo: "Unicode('-')" },
	{ code: "Equal", label: "=", mac: 0x18, enigo: "Unicode('=')" },
	{ code: "BracketLeft", label: "[", mac: 0x21, enigo: "Unicode('[')" },
	{ code: "BracketRight", label: "]", mac: 0x1e, enigo: "Unicode(']')" },
	{ code: "Backslash", label: "\\", mac: 0x2a, enigo: "Unicode('\\\\')" },
	{ code: "Semicolon", label: ";", mac: 0x29, enigo: "Unicode(';')" },
	{ code: "Quote", label: "'", mac: 0x27, enigo: "Unicode('\\'')" },
	{ code: "Comma", label: ",", mac: 0x2b, enigo: "Unicode(',')" },
	{ code: "Period", label: ".", mac: 0x2f, enigo: "Unicode('.')" },
	{ code: "Slash", label: "/", mac: 0x2c, enigo: "Unicode('/')" },
	{ code: "Backquote", label: "`", mac: 0x32, enigo: "Unicode('`')" },
	{ code: "IntlBackslash", label: "§", mac: 0x0a, enigo: "Unicode('§')" },
	{ code: "Enter", label: "Return", mac: 0x24, enigo: "Return" },
	{ code: "Tab", label: "Tab", mac: 0x30, enigo: "Tab" },
	{ code: "Space", label: "Space", mac: 0x31, enigo: "Space" },
	{ code: "Backspace", label: isMac ? "Delete" : "Backspace", mac: 0x33, enigo: "Backspace" },
	{ code: "Delete", label: isMac ? "Forward Delete" : "Delete", mac: 0x75, enigo: "Delete" },
	{ code: "Escape", label: "Esc", mac: 0x35, enigo: "Escape" },
	{ code: "Home", label: "Home", mac: 0x73, enigo: "Home" },
	{ code: "End", label: "End", mac: 0x77, enigo: "End" },
	{ code: "PageUp", label: "Page Up", mac: 0x74, enigo: "PageUp" },
	{ code: "PageDown", label: "Page Down", mac: 0x79, enigo: "PageDown" },
	{ code: "ArrowLeft", label: "←", mac: 0x7b, enigo: "LeftArrow" },
	{ code: "ArrowRight", label: "→", mac: 0x7c, enigo: "RightArrow" },
	{ code: "ArrowDown", label: "↓", mac: 0x7d, enigo: "DownArrow" },
	{ code: "ArrowUp", label: "↑", mac: 0x7e, enigo: "UpArrow" },
	...[
		["F1", 0x7a],
		["F2", 0x78],
		["F3", 0x63],
		["F4", 0x76],
		["F5", 0x60],
		["F6", 0x61],
		["F7", 0x62],
		["F8", 0x64],
		["F9", 0x65],
		["F10", 0x6d],
		["F11", 0x67],
		["F12", 0x6f],
		["F13", 0x69],
		["F14", 0x6b],
		["F15", 0x71],
		["F16", 0x6a],
		["F17", 0x40],
		["F18", 0x4f],
		["F19", 0x50],
		["F20", 0x5a],
	].map(([code, mac]) => ({ code: code as string, label: code as string, mac: mac as number, enigo: code as string })),
	...[
		["Numpad0", 0x52],
		["Numpad1", 0x53],
		["Numpad2", 0x54],
		["Numpad3", 0x55],
		["Numpad4", 0x56],
		["Numpad5", 0x57],
		["Numpad6", 0x58],
		["Numpad7", 0x59],
		["Numpad8", 0x5b],
		["Numpad9", 0x5c],
	].map(([code, mac]) => ({ code: code as string, label: `Num ${(code as string).slice(6)}`, mac: mac as number, enigo: code as string })),
	{ code: "NumpadAdd", label: "Num +", mac: 0x45, enigo: "Add" },
	{ code: "NumpadSubtract", label: "Num −", mac: 0x4e, enigo: "Subtract" },
	{ code: "NumpadMultiply", label: "Num ×", mac: 0x43, enigo: "Multiply" },
	{ code: "NumpadDivide", label: "Num ÷", mac: 0x4b, enigo: "Divide" },
	{ code: "NumpadDecimal", label: "Num .", mac: 0x41, enigo: "Decimal" },
	{ code: "NumpadEnter", label: "Num Enter", mac: 0x4c, enigo: "Return" },
];

const BY_CODE = new Map(KEY_TABLE.map((key) => [key.code, key]));
const BY_MAC = new Map(KEY_TABLE.map((key) => [key.mac, key]));
const BY_ENIGO = new Map(KEY_TABLE.map((key) => [key.enigo.toLowerCase(), key]));

/** Keys offered by the manual picker, grouped for the dropdown. */
export const PICKABLE_KEYS: { group: string; keys: { code: string; label: string }[] }[] = [
	{ group: "Letters", keys: KEY_TABLE.filter((key) => key.code.startsWith("Key")) },
	{ group: "Numbers", keys: KEY_TABLE.filter((key) => key.code.startsWith("Digit")) },
	{ group: "Function keys", keys: KEY_TABLE.filter((key) => /^F\d+$/.test(key.code)) },
	{ group: "Navigation", keys: KEY_TABLE.filter((key) => ["Enter", "Tab", "Space", "Backspace", "Delete", "Escape", "Home", "End", "PageUp", "PageDown", "ArrowLeft", "ArrowRight", "ArrowDown", "ArrowUp"].includes(key.code)) },
	{ group: "Symbols", keys: KEY_TABLE.filter((key) => ["Minus", "Equal", "BracketLeft", "BracketRight", "Backslash", "Semicolon", "Quote", "Comma", "Period", "Slash", "Backquote"].includes(key.code)) },
	{ group: "Numeric keypad", keys: KEY_TABLE.filter((key) => key.code.startsWith("Numpad")) },
];

export type Shortcut = { modifiers: Modifier[]; code: string };

export function keyLabel(code: string): string {
	return BY_CODE.get(code)?.label ?? code;
}

/** Human-readable form, like ⌃⌥⇧⌘K or Ctrl + Shift + K. */
export function formatShortcut(shortcut: Shortcut): string {
	const modifiers = MODIFIERS.filter((modifier) => shortcut.modifiers.includes(modifier)).map((modifier) => MODIFIER_SYMBOLS[modifier]);
	const key = keyLabel(shortcut.code);
	return isMac ? [...modifiers, key].join("") : [...modifiers, key].join(" + ");
}

function keyToken(code: string, direction: "Press" | "Release" | "Click"): string {
	const key = BY_CODE.get(code);
	if (!key) throw new Error(`Unsupported key: ${code}`);
	return isMac ? (direction === "Click" ? `r(${key.mac})` : `r(${key.mac},${direction})`) : `k(${key.enigo},${direction})`;
}

/** A complete tap of the shortcut: press modifiers, tap the key, release modifiers. */
export function shortcutSequence(shortcut: Shortcut): string {
	const modifiers = MODIFIERS.filter((modifier) => shortcut.modifiers.includes(modifier));
	return `[${[...modifiers.map((modifier) => `k(${modifier},Press)`), keyToken(shortcut.code, "Click"), ...[...modifiers].reverse().map((modifier) => `k(${modifier},Release)`)].join(",")}]`;
}

/** Press-and-hold: the press half and the release half, for Super Hotkey. */
export function shortcutHoldSequences(shortcut: Shortcut): { down: string; up: string } {
	const modifiers = MODIFIERS.filter((modifier) => shortcut.modifiers.includes(modifier));
	return {
		down: `[${[...modifiers.map((modifier) => `k(${modifier},Press)`), keyToken(shortcut.code, "Press")].join(",")}]`,
		up: `[${[keyToken(shortcut.code, "Release"), ...[...modifiers].reverse().map((modifier) => `k(${modifier},Release)`)].join(",")}]`,
	};
}

/** Split a RON token list into its top-level tokens. */
function splitTokens(sequence: string): string[] | null {
	const body = sequence.trim();
	if (!body.startsWith("[") || !body.endsWith("]")) return null;
	const tokens: string[] = [];
	let depth = 0;
	let quote: string | null = null;
	let current = "";
	for (let index = 1; index < body.length - 1; index++) {
		const character = body[index];
		if (quote) {
			current += character;
			if (character === "\\") {
				current += body[++index] ?? "";
			} else if (character === quote) {
				quote = null;
			}
			continue;
		}
		if (character === '"' || character === "'") {
			quote = character;
			current += character;
		} else if (character === "(") {
			depth++;
			current += character;
		} else if (character === ")") {
			depth--;
			current += character;
		} else if (character === "," && depth === 0) {
			if (current.trim()) tokens.push(current.trim());
			current = "";
		} else {
			current += character;
		}
	}
	if (current.trim()) tokens.push(current.trim());
	return tokens;
}

const MODIFIER_ALIASES: Record<string, Modifier> = {
	meta: "Meta",
	command: "Meta",
	cmd: "Meta",
	rcommand: "Meta",
	control: "Control",
	ctrl: "Control",
	lcontrol: "Control",
	rcontrol: "Control",
	alt: "Alt",
	option: "Alt",
	roption: "Alt",
	shift: "Shift",
	lshift: "Shift",
	rshift: "Shift",
};

/**
 * Describe a saved sequence as keycaps, for display. Returns null when the
 * sequence is something other than a single shortcut (text, several keys,
 * mouse input); the raw sequence is shown then.
 */
export function describeSequence(sequence: string): { modifiers: Modifier[]; key: string } | null {
	const tokens = splitTokens(sequence);
	if (!tokens || tokens.length === 0) return null;
	const modifiers: Modifier[] = [];
	let key: string | null = null;
	for (const token of tokens) {
		const match = /^([kKrR])\((.*)\)$/.exec(token);
		if (!match) return null;
		const [first, direction = "Click"] = match[2].split(/,(?![^(]*\))/).map((part) => part.trim());
		if (match[1].toLowerCase() === "r") {
			const code = Number(first);
			if (!Number.isFinite(code)) return null;
			const known = isMac ? BY_MAC.get(code) : undefined;
			if (/release/i.test(direction)) continue;
			if (key) return null;
			key = known?.label ?? `Key ${code}`;
			continue;
		}
		const modifier = MODIFIER_ALIASES[first.toLowerCase()];
		if (modifier) {
			if (/press/i.test(direction) && !modifiers.includes(modifier)) modifiers.push(modifier);
			continue;
		}
		if (/release/i.test(direction)) continue;
		if (key) return null;
		const known = BY_ENIGO.get(first.toLowerCase());
		const unicode = /^(?:Unicode|uni|Char|char)\('(.+)'\)$/.exec(first);
		key = known?.label ?? (unicode ? unicode[1].toUpperCase() : first);
	}
	return key ? { modifiers: MODIFIERS.filter((modifier) => modifiers.includes(modifier)), key } : null;
}

export function describeSequenceText(sequence: string): string {
	const described = describeSequence(sequence);
	if (!described) return "";
	const modifiers = described.modifiers.map((modifier) => MODIFIER_SYMBOLS[modifier]);
	return isMac ? [...modifiers, described.key].join("") : [...modifiers, described.key].join(" + ");
}

/** The shortcut for a keydown, or null for a lone modifier. */
export function shortcutFromEvent(event: KeyboardEvent): Shortcut | null {
	if (["MetaLeft", "MetaRight", "ControlLeft", "ControlRight", "AltLeft", "AltRight", "ShiftLeft", "ShiftRight", "CapsLock", "Fn"].includes(event.code)) return null;
	if (!BY_CODE.has(event.code)) return null;
	const modifiers: Modifier[] = [];
	if (event.ctrlKey) modifiers.push("Control");
	if (event.altKey) modifiers.push("Alt");
	if (event.shiftKey) modifiers.push("Shift");
	if (event.metaKey) modifiers.push("Meta");
	return { modifiers, code: event.code };
}

export function modifiersFromEvent(event: KeyboardEvent): Modifier[] {
	const modifiers: Modifier[] = [];
	if (event.ctrlKey) modifiers.push("Control");
	if (event.altKey) modifiers.push("Alt");
	if (event.shiftKey) modifiers.push("Shift");
	if (event.metaKey) modifiers.push("Meta");
	return modifiers;
}
