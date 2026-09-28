import type { ActionState } from "./ActionState.ts";
import type { Context } from "./Context.ts";

import { getWebserverUrl } from "./ports.ts";

import { invoke } from "@tauri-apps/api/core";

export function getImage(image: string | undefined, fallback: string | undefined): string {
	if (!image) return fallback ? getImage(fallback, undefined) : "/alert.png";
	if (image.startsWith("opendeck/")) return image.replace("opendeck", "");
	if (!image.startsWith("data:")) return getWebserverUrl(image);
	const svgxmlre = /^data:image\/svg\+xml(?!.*?;base64.*?)(?:;[\w=]*)*,(.+)/;
	const base64re = /^data:image\/(apng|avif|gif|jpeg|png|svg\+xml|webp|bmp|x-icon|tiff);base64,([A-Za-z0-9+/]+={0,2})?/;
	if (svgxmlre.test(image)) {
		let svg = (svgxmlre.exec(image) as RegExpExecArray)[1].replace(/\;$/, "");
		try {
			svg = decodeURIComponent(svg);
		} finally {
			image = "data:image/svg+xml," + encodeURIComponent(svg);
		}
	}
	if (base64re.test(image)) {
		const exec = base64re.exec(image)!;
		if (!exec[2]) return fallback ? getImage(fallback, undefined) : "/alert.png";
		else image = exec[0];
	}
	return image;
}

/**
 * Built-in key artwork has a variant with the icon raised to make room for a
 * title along the bottom; use it whenever such a title is shown.
 */
export function artworkForState(image: string, state: Pick<ActionState, "show" | "text" | "alignment">): string {
	const face = /^opendeck\/keys\/([a-z0-9-]+)\.svg$/.exec(image);
	if (face && state.show && state.text.trim() && state.alignment === "bottom") return `opendeck/keys/titled/${face[1]}.svg`;
	return image;
}

/**
 * Load an image, giving up after a few seconds and trying once more, so a
 * load that never finishes (a stuck connection to the image server) cannot
 * leave a key undrawn: a key is redrawn only after its last drawing ends.
 */
export async function loadImage(source: string, timeout = 4000): Promise<HTMLImageElement> {
	for (let attempt = 1; ; attempt++) {
		const image = document.createElement("img");
		image.crossOrigin = "anonymous";
		const loaded = new Promise<HTMLImageElement>((resolve, reject) => {
			image.onload = () => resolve(image);
			image.onerror = () => reject(new Error(`Could not load ${source.slice(0, 120)}`));
		});
		// A retry asks the server again rather than waiting on the same request.
		image.src = attempt > 1 && !source.startsWith("data:") ? `${source}${source.includes("?") ? "&" : "?"}retry=${Date.now()}` : source;
		let timer: ReturnType<typeof setTimeout> | undefined;
		const timedOut = new Promise<never>((_, reject) => {
			timer = setTimeout(() => reject(new Error(`Timed out loading ${source.slice(0, 120)}`)), timeout);
		});
		try {
			return await Promise.race([loaded, timedOut]);
		} catch (error) {
			image.src = "";
			if (attempt >= 2) throw error;
		} finally {
			clearTimeout(timer);
		}
	}
}

export class CanvasLock {
	currentLock = Promise.resolve();
	async lock() {
		let unlockNext: () => void;
		const willLock = new Promise<void>((resolve) => (unlockNext = resolve));
		const previousLock = this.currentLock;
		this.currentLock = willLock;
		await previousLock;
		return unlockNext!;
	}
}

export async function renderImage(
	canvas: HTMLCanvasElement | null,
	slotContext: Context | null,
	state: ActionState,
	fallback: string | undefined,
	showOk: boolean,
	showAlert: boolean,
	processImage: boolean,
	active: boolean,
	pressed: boolean,
	rotation?: number,
	/** Wait until the backend has stored this finished image. Used by page warming. */
	awaitDeviceUpdate = false,
) {
	// Create canvas
	let scale = 1;
	if (!canvas) {
		canvas = document.createElement("canvas");
		canvas.width = 144;
		canvas.height = 144;
	} else {
		// Use height for scale to handle rectangular infobar canvases
		scale = canvas.height / 144;
	}

	const context = canvas.getContext("2d");
	if (!context) return;

	context.save();
	if (rotation) {
		context.translate(canvas.width / 2, canvas.height / 2);
		context.rotate((rotation * Math.PI) / 180);
		context.translate(-canvas.width / 2, -canvas.height / 2);
	}

	try {
		// Load image
		const source = processImage ? getImage(artworkForState(state.image || fallback || "", state), fallback) : state.image;
		if (source == undefined) return;
		const image = await loadImage(source);

		context.clearRect(0, 0, canvas.width, canvas.height);

		// Draw background color
		if (!state.background_colour.startsWith("#000000")) {
			context.fillStyle = state.background_colour;
			context.fillRect(0, 0, canvas.width, canvas.height);
		}

		// Draw image
		context.imageSmoothingQuality = "high";
		const imageScale = Math.max(10, state.image_scale || 100) / 100;
		const xScaled = canvas.width * imageScale;
		const yScaled = canvas.height * imageScale;
		const xOffset = (canvas.width - xScaled) / 2;
		let yOffset = (canvas.height - yScaled) / 2;
		// A reduced image sits centred in the space above a bottom title.
		if (imageScale < 1 && state.show && state.text.trim() && state.alignment === "bottom") {
			const titleHeight = state.size * 2 * scale * state.text.split("\n").length + state.stroke_size * scale * 2;
			yOffset = Math.max(canvas.height * 0.04, (canvas.height - titleHeight - yScaled) / 2);
		}
		context.drawImage(image, xOffset, yOffset, xScaled, yScaled);
	} catch (error: any) {
		if (!(error instanceof Event)) console.error(error);
		context.clearRect(0, 0, canvas.width, canvas.height);
		showAlert = true;
	}

	// Draw text
	if (state.show) {
		const font = (size: number) => (state.style.includes("Bold") ? "bold " : "") + (state.style.includes("Italic") ? "italic " : "") + `${size}px "${state.family}", sans-serif`;
		let size = state.size * 2 * scale;
		context.textAlign = "center";
		context.font = font(size);
		// A long title shrinks (to half its size at most) to fit the key
		// instead of running off both edges.
		const room = canvas.width - 2 * (state.stroke_size + 3) * scale;
		const widest = Math.max(...state.text.split("\n").map((line) => context.measureText(line).width));
		if (widest > room) {
			size = Math.max(size / 2, (size * room) / widest);
			context.font = font(size);
		}
		context.fillStyle = state.colour;
		context.strokeStyle = state.stroke_colour;
		context.lineWidth = state.stroke_size * scale;
		// Mitred corners spike out of letters with sharp angles such as M and V.
		context.lineJoin = "round";
		context.textBaseline = "top";
		const x = canvas.width / 2;
		let y = canvas.height / 2 - size * state.text.split("\n").length * 0.5;
		switch (state.alignment) {
			case "top":
				y = context.lineWidth;
				break;
			case "bottom":
				y = canvas.height - size * state.text.split("\n").length - context.lineWidth;
				break;
		}
		for (const [index, line] of Object.entries(state.text.split("\n"))) {
			context.strokeText(line, x, y + size * parseInt(index));
			context.fillText(line, x, y + size * parseInt(index));
			if (state.underline) {
				const width = context.measureText(line).width;
				// Set to black for the outline, since it uses the same fill style info as the text colour.
				context.fillStyle = "black";
				context.fillRect(x - width / 2 - 3, y + size * parseInt(index) + size, width + 6, 9);
				// Reset to the user's choice of text colour.
				context.fillStyle = state.colour;
				context.fillRect(x - width / 2, y + size * parseInt(index) + size + 4, width, 3);
			}
		}
	}

	for (const [shown, overlay] of [
		[showOk, "/ok.png"],
		[showAlert, "/alert.png"],
	] as const) {
		if (!shown) continue;
		try {
			context.drawImage(await loadImage(overlay), 0, 0, canvas.width, canvas.height);
		} catch (error) {
			console.warn(error);
		}
	}

	// Make the image smaller while the button is pressed.
	if (pressed) {
		const smallCanvas = document.createElement("canvas");
		smallCanvas.width = canvas.width;
		smallCanvas.height = canvas.height;
		const newContext = smallCanvas.getContext("2d");
		const margin = 0.1;
		if (newContext) {
			newContext.drawImage(canvas, canvas.width * margin, canvas.height * margin, canvas.width * (1 - margin * 2), canvas.height * (1 - margin * 2));
			context.clearRect(0, 0, canvas.width, canvas.height);
			context.drawImage(smallCanvas, 0, 0);
		}
	}

	context.restore();

	if (active && slotContext) {
		const update = invoke("update_image", { context: slotContext, image: canvas.toDataURL("image/jpeg") });
		if (awaitDeviceUpdate) await update;
		else void update;
	}
}

export async function resizeImage(source: string): Promise<string | undefined> {
	const canvas = document.createElement("canvas");
	canvas.width = 288;
	canvas.height = 288;
	const context = canvas.getContext("2d");
	if (!context) return;

	let image: HTMLImageElement;
	try {
		image = await loadImage(source);
	} catch {
		return undefined;
	}

	let xOffset = 0,
		yOffset = 0;
	let xScaled = canvas.width,
		yScaled = canvas.height;
	if (image.width > image.height) {
		const ratio = image.height / image.width;
		yScaled = canvas.height * ratio;
		yOffset = (canvas.height - yScaled) / 2;
	} else if (image.width < image.height) {
		const ratio = image.width / image.height;
		xScaled = canvas.width * ratio;
		xOffset = (canvas.width - xScaled) / 2;
	}

	context.imageSmoothingQuality = "high";
	context.clearRect(0, 0, canvas.width, canvas.height);
	context.drawImage(image, xOffset, yOffset, xScaled, yScaled);

	return canvas.toDataURL();
}
