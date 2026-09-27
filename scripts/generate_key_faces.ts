// Generates the default key artwork in `static/keys/`.
//
// Each face is a 144×144 SVG: a gradient in its library group's colour, a soft
// highlight, and a bold white icon. The editor renders faces onto the key
// canvas and sends the result to the M18, whose LCD keys are 64×64, so icons
// are large, filled, and high-contrast.
//
// Icons are from Phosphor Icons (https://phosphoricons.com), MIT License,
// read from the `phosphor-svelte` package that the editor already uses.
//
// Run from the repository root: deno run --allow-read --allow-write scripts/generate_key_faces.ts

type Gradient = [string, string] | string[];

const GROUPS: Record<string, Gradient> = {
	apps: ["#60A5FA", "#1D4ED8"],
	keyboard: ["#A78BFA", "#6D28D9"],
	media: ["#F472B6", "#BE185D"],
	system: ["#2DD4BF", "#0F766E"],
	display: ["#FBBF24", "#B45309"],
	pages: ["#FB923C", "#C2410C"],
	flows: ["#4ADE80", "#15803D"],
	device: ["#F87171", "#991B1B"],
	browser: ["#38BDF8", "#0369A1"],
	premiere: ["#818CF8", "#3730A3"],
	network: ["#22D3EE", "#0E7490"],
	soon: ["#94A3B8", "#475569"],
	rainbow: ["#F43F5E", "#F59E0B", "#22C55E", "#0EA5E9", "#8B5CF6"],
};

// "badge" draws a small icon at the top, leaving the centre for a number that
// the editor draws over it (Go to Page and Page Number).
type Face = { group: keyof typeof GROUPS; icon: string; layout?: "badge" };

const FACES: Record<string, Face> = {
	// Apps & websites
	"open-app": { group: "apps", icon: "AppWindow" },
	"open-file": { group: "apps", icon: "FolderOpen" },
	"open-website": { group: "apps", icon: "Globe" },
	"quit-app": { group: "apps", icon: "XCircle" },
	calculator: { group: "apps", icon: "Calculator" },
	mail: { group: "apps", icon: "EnvelopeSimple" },
	"music-app": { group: "apps", icon: "MusicNotesSimple" },
	"activity-monitor": { group: "apps", icon: "Pulse" },
	"system-settings": { group: "apps", icon: "GearSix" },
	safari: { group: "apps", icon: "Compass" },
	// Keyboard & text
	hotkey: { group: "keyboard", icon: "Command" },
	"hotkey-switch": { group: "keyboard", icon: "Swap" },
	"super-hotkey": { group: "keyboard", icon: "Keyboard" },
	"super-hotkey-switch": { group: "keyboard", icon: "ArrowsLeftRight" },
	"type-text": { group: "keyboard", icon: "TextT" },
	password: { group: "keyboard", icon: "Password" },
	emoji: { group: "keyboard", icon: "Smiley" },
	mouse: { group: "keyboard", icon: "CursorClick" },
	// Media & audio
	"play-pause": { group: "media", icon: "PlayPause" },
	"previous-track": { group: "media", icon: "SkipBack" },
	"next-track": { group: "media", icon: "SkipForward" },
	"fast-forward": { group: "media", icon: "FastForward" },
	rewind: { group: "media", icon: "Rewind" },
	"volume-up": { group: "media", icon: "SpeakerHigh" },
	"volume-down": { group: "media", icon: "SpeakerLow" },
	mute: { group: "media", icon: "SpeakerSlash" },
	microphone: { group: "media", icon: "MicrophoneSlash" },
	"media-key": { group: "media", icon: "SlidersHorizontal" },
	"play-sound": { group: "media", icon: "Waveform" },
	"stop-sounds": { group: "media", icon: "StopCircle" },
	// System
	siri: { group: "system", icon: "Sparkle" },
	"mission-control": { group: "system", icon: "SquaresFour" },
	launchpad: { group: "system", icon: "DotsNine" },
	spotlight: { group: "system", icon: "MagnifyingGlass" },
	screenshot: { group: "system", icon: "Scan" },
	"show-desktop": { group: "system", icon: "Desktop" },
	dictation: { group: "system", icon: "Microphone" },
	"input-source": { group: "system", icon: "Translate" },
	"emoji-viewer": { group: "system", icon: "Smiley" },
	focus: { group: "system", icon: "BellSlash" },
	notifications: { group: "system", icon: "Bell" },
	// Display & power
	"brightness-up": { group: "display", icon: "Sun" },
	"brightness-down": { group: "display", icon: "SunDim" },
	"display-sleep": { group: "display", icon: "MoonStars" },
	"screen-saver": { group: "display", icon: "MonitorPlay" },
	"lock-screen": { group: "display", icon: "LockSimple" },
	// Pages & folders
	"page-next": { group: "pages", icon: "CaretRight" },
	"page-previous": { group: "pages", icon: "CaretLeft" },
	"page-goto": { group: "pages", icon: "Stack", layout: "badge" },
	"page-indicator": { group: "pages", icon: "Hash", layout: "badge" },
	"open-folder": { group: "pages", icon: "Folder" },
	"go-back": { group: "pages", icon: "ArrowUUpLeft" },
	"scene-shift": { group: "pages", icon: "ArrowsCounterClockwise" },
	// Action flows
	"multi-action": { group: "flows", icon: "Stack" },
	"action-cycle": { group: "flows", icon: "Repeat" },
	"action-carousel": { group: "flows", icon: "ArrowsClockwise" },
	delay: { group: "flows", icon: "Hourglass" },
	// M18 device
	"led-colors": { group: "rainbow", icon: "Lightbulb" },
	"m18-brightness": { group: "device", icon: "SunHorizon" },
	// Browser
	"browser-back": { group: "browser", icon: "ArrowLeft" },
	"browser-forward": { group: "browser", icon: "ArrowRight" },
	"browser-reload": { group: "browser", icon: "ArrowClockwise" },
	bookmark: { group: "browser", icon: "BookmarkSimple" },
	// Premiere Pro
	"pr-play": { group: "premiere", icon: "Play" },
	"pr-add-edit": { group: "premiere", icon: "SplitHorizontal" },
	"pr-razor": { group: "premiere", icon: "Knife" },
	"pr-ripple": { group: "premiere", icon: "ArrowsInLineHorizontal" },
	"pr-pen": { group: "premiere", icon: "PenNib" },
	"pr-rectangle": { group: "premiere", icon: "Rectangle" },
	"pr-marker": { group: "premiere", icon: "MapPin" },
	"pr-effects": { group: "premiere", icon: "MagicWand" },
	"pr-fit": { group: "premiere", icon: "MagnifyingGlassMinus" },
	"pr-fullscreen": { group: "premiere", icon: "ArrowsOut" },
	"pr-cut": { group: "premiere", icon: "Scissors" },
	"pr-copy": { group: "premiere", icon: "Copy" },
	"pr-paste": { group: "premiere", icon: "ClipboardText" },
	"pr-delete": { group: "premiere", icon: "Trash" },
	"pr-undo": { group: "premiere", icon: "ArrowCounterClockwise" },
	// Network
	udp: { group: "network", icon: "Broadcast" },
	// Coming soon
	timer: { group: "soon", icon: "Timer" },
	countdown: { group: "soon", icon: "HourglassMedium" },
	"world-clock": { group: "soon", icon: "GlobeHemisphereWest" },
	"date-time": { group: "soon", icon: "Clock" },
	calendar: { group: "soon", icon: "CalendarBlank" },
	weather: { group: "soon", icon: "CloudSun" },
	notes: { group: "soon", icon: "NotePencil" },
	todo: { group: "soon", icon: "ListChecks" },
	"youtube-chat": { group: "soon", icon: "ChatText" },
	"youtube-viewers": { group: "soon", icon: "Eye" },
	vmix: { group: "soon", icon: "VideoCamera" },
	"focus-search": { group: "soon", icon: "MagnifyingGlassPlus" },
	stickers: { group: "soon", icon: "Sticker" },
	paint: { group: "soon", icon: "PaintBrush" },
	"paint-full": { group: "soon", icon: "PaintBucket" },
	rhythm: { group: "soon", icon: "MusicNote" },
	game: { group: "soon", icon: "GameController" },
	water: { group: "soon", icon: "Drop" },
	"coming-soon": { group: "soon", icon: "Hourglass" },
	unsupported: { group: "soon", icon: "Question" },
};

const PHOSPHOR = "node_modules/phosphor-svelte/lib";

async function iconPaths(name: string): Promise<string> {
	const source = await Deno.readTextFile(`${PHOSPHOR}/${name}/${name}.svelte`);
	const start = source.indexOf('weight === "fill"}');
	const end = source.indexOf("{:else", start);
	if (start < 0 || end < 0) throw new Error(`No filled variant for ${name}`);
	const paths = source.slice(start, end).match(/<path[^>]*\/>/g);
	if (!paths) throw new Error(`No paths for ${name}`);
	return paths.join("");
}

function gradientStops(colors: Gradient): string {
	return colors.map((color, index) => `<stop offset="${(index / (colors.length - 1)).toFixed(2)}" stop-color="${color}"/>`).join("");
}

function face(paths: string, colors: Gradient, layout?: "badge" | "titled"): string {
	// Icon size in key pixels, and its top-left corner. "titled" raises and
	// shrinks the icon to leave room for a title along the bottom.
	const size = layout === "badge" ? 40 : layout === "titled" ? 62 : 78;
	const x = (144 - size) / 2;
	const y = layout === "badge" ? 12 : layout === "titled" ? 16 : (144 - size) / 2;
	const scale = (size / 256).toFixed(5);
	return `<svg xmlns="http://www.w3.org/2000/svg" width="144" height="144" viewBox="0 0 144 144">
<!-- Icon: Phosphor Icons (https://phosphoricons.com), MIT License -->
<defs>
<linearGradient id="background" x1="0" y1="0" x2="1" y2="1">${gradientStops(colors)}</linearGradient>
<radialGradient id="highlight" cx="0.28" cy="0.1" r="0.95"><stop offset="0" stop-color="#FFFFFF" stop-opacity="0.32"/><stop offset="0.6" stop-color="#FFFFFF" stop-opacity="0"/></radialGradient>
</defs>
<rect width="144" height="144" fill="url(#background)"/>
<rect width="144" height="144" fill="url(#highlight)"/>
<g transform="translate(${x} ${y + 3}) scale(${scale})" fill="#000000" fill-opacity="0.22">${paths}</g>
<g transform="translate(${x} ${y}) scale(${scale})" fill="#FFFFFF">${paths}</g>
</svg>
`;
}

await Deno.mkdir("static/keys/titled", { recursive: true });
for (const [slug, { group, icon, layout }] of Object.entries(FACES)) {
	const paths = await iconPaths(icon);
	await Deno.writeTextFile(`static/keys/${slug}.svg`, face(paths, GROUPS[group], layout));
	// Used by the editor when the key shows a title along the bottom.
	await Deno.writeTextFile(`static/keys/titled/${slug}.svg`, face(paths, GROUPS[group], layout ?? "titled"));
}
console.log(`Wrote ${Object.keys(FACES).length} key faces (and titled variants) to static/keys/`);
