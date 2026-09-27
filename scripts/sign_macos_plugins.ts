/// <reference lib="deno.ns" />

// Signs the programs of the built-in plugins before `tauri build` packs them
// into the app. Tauri signs the app and its sidecars but only copies
// resources, and notarization rejects an app that contains a program without
// a Developer ID signature, the hardened runtime, and a secure timestamp.
//
// Runs as Tauri's `beforeBundleCommand`. It does nothing unless
// APPLE_SIGNING_IDENTITY names a real signing identity on macOS.

const identity = Deno.env.get("APPLE_SIGNING_IDENTITY")?.trim();
if (Deno.build.os !== "darwin" || !identity || identity === "-") Deno.exit(0);

const PLUGINS = decodeURIComponent(new URL("../src-tauri/target/plugins", import.meta.url).pathname);
// 32- and 64-bit Mach-O in either byte order, and universal binaries.
const MACH_O = [0xfeedface, 0xcefaedfe, 0xfeedfacf, 0xcffaedfe, 0xcafebabe, 0xbebafeca];

async function isMachO(path: string): Promise<boolean> {
	const file = await Deno.open(path);
	try {
		const magic = new Uint8Array(4);
		return (await file.read(magic)) === 4 && MACH_O.includes(new DataView(magic.buffer).getUint32(0));
	} finally {
		file.close();
	}
}

async function* programs(directory: string): AsyncGenerator<string> {
	for await (const entry of Deno.readDir(directory)) {
		const path = `${directory}/${entry.name}`;
		if (entry.isDirectory) yield* programs(path);
		else if (entry.isFile && (await isMachO(path))) yield path;
	}
}

try {
	await Deno.stat(PLUGINS);
} catch {
	console.log("No built-in plugins to sign.");
	Deno.exit(0);
}

for await (const path of programs(PLUGINS)) {
	const { code, stderr } = await new Deno.Command("codesign", {
		args: ["--force", "--sign", identity, "--options", "runtime", "--timestamp", path],
	}).output();
	if (code !== 0) {
		console.error(new TextDecoder().decode(stderr).trim());
		throw new Error(`Failed to sign ${path}`);
	}
	console.log(`Signed ${path.slice(PLUGINS.length + 1)}`);
}
