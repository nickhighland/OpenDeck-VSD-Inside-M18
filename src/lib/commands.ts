/**
 * Ready-made commands for Run Command keys and command entries in switches.
 * Commands run in the background with /bin/sh, so they also work while the
 * Mac is locked; Homebrew's folders are on the PATH.
 */
export const COMMAND_PRESETS: { group: string; items: { label: string; command: string }[] }[] = [
	{
		// m1ddc (brew install m1ddc) switches inputs over DDC/CI. Monitors usually
		// follow the VESA input codes below; a few use others.
		group: "Monitor input (m1ddc)",
		items: [
			{ label: "DisplayPort", command: "m1ddc set input 15" },
			{ label: "HDMI 1", command: "m1ddc set input 17" },
			{ label: "HDMI 2", command: "m1ddc set input 18" },
			{ label: "USB-C", command: "m1ddc set input 27" },
		],
	},
];

/** The preset's label for a command, if it is one. */
export function presetLabel(command: string): string | undefined {
	for (const group of COMMAND_PRESETS) {
		const preset = group.items.find((item) => item.command === command.trim());
		if (preset) return preset.label;
	}
	return undefined;
}
