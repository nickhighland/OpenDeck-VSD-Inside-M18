<script lang="ts">
	import { convertFileSrc, invoke } from "@tauri-apps/api/core";
	import { listen, type UnlistenFn } from "@tauri-apps/api/event";
	import { onMount } from "svelte";

	import { settings } from "$lib/settings";

	const LCD_COLUMNS = 5;
	const LCD_ROWS = 3;
	const LCD_SIZE = 64;
	const FRAME_WIDTH = 480;
	const FRAME_HEIGHT = 272;
	// The 64-pixel key windows are centred on a full-width M18 display.
	const KEY_STEP_X = FRAME_WIDTH / LCD_COLUMNS;
	const KEY_STEP_Y = FRAME_HEIGHT / LCD_ROWS;
	const FRAME_INTERVAL_MS = 160;

	let activeDevice: string | null = null;
	let frameTimer: number | undefined;
	let video: HTMLVideoElement | undefined;
	let frameGeneration = 0;
	let sendingFrame = false;

	const frameCanvas = document.createElement("canvas");
	frameCanvas.width = FRAME_WIDTH;
	frameCanvas.height = FRAME_HEIGHT;
	const tileCanvas = document.createElement("canvas");
	tileCanvas.width = LCD_SIZE;
	tileCanvas.height = LCD_SIZE;

	function waitForImage(url: string): Promise<HTMLImageElement> {
		const image = new Image();
		image.crossOrigin = "anonymous";
		image.src = convertFileSrc(url);
		return new Promise((resolve, reject) => {
			image.onload = () => resolve(image);
			image.onerror = () => reject(new Error(`Unable to load screensaver image: ${url}`));
		});
	}

	function waitForVideo(element: HTMLVideoElement): Promise<void> {
		return new Promise((resolve, reject) => {
			if (element.readyState >= HTMLMediaElement.HAVE_METADATA) {
				resolve();
				return;
			}
			element.addEventListener("loadedmetadata", () => resolve(), { once: true });
			element.addEventListener("error", () => reject(new Error("Unable to load screensaver video")), { once: true });
		});
	}

	function drawCover(source: CanvasImageSource, sourceWidth: number, sourceHeight: number) {
		const context = frameCanvas.getContext("2d");
		if (!context || !sourceWidth || !sourceHeight) return false;

		const scale = Math.max(FRAME_WIDTH / sourceWidth, FRAME_HEIGHT / sourceHeight);
		const width = sourceWidth * scale;
		const height = sourceHeight * scale;
		context.fillStyle = "#000000";
		context.fillRect(0, 0, FRAME_WIDTH, FRAME_HEIGHT);
		context.drawImage(source, (FRAME_WIDTH - width) / 2, (FRAME_HEIGHT - height) / 2, width, height);
		return true;
	}

	async function sendFrame(source: CanvasImageSource, sourceWidth: number, sourceHeight: number, generation: number) {
		if (generation !== frameGeneration || !activeDevice || sendingFrame || !drawCover(source, sourceWidth, sourceHeight)) return;

		const context = tileCanvas.getContext("2d");
		if (!context) return;

		const images: string[] = [];
		for (let row = 0; row < LCD_ROWS; row++) {
			for (let column = 0; column < LCD_COLUMNS; column++) {
				context.clearRect(0, 0, LCD_SIZE, LCD_SIZE);
				const x = Math.round(column * KEY_STEP_X + (KEY_STEP_X - LCD_SIZE) / 2);
				const y = Math.round(row * KEY_STEP_Y + (KEY_STEP_Y - LCD_SIZE) / 2);
				context.drawImage(frameCanvas, x, y, LCD_SIZE, LCD_SIZE, 0, 0, LCD_SIZE, LCD_SIZE);
				images.push(tileCanvas.toDataURL("image/jpeg", 0.78));
			}
		}

		sendingFrame = true;
		try {
			await invoke("set_screensaver_frame", { device: activeDevice, background: frameCanvas.toDataURL("image/jpeg", 0.78), images });
		} catch (error) {
			console.error("M18 screensaver frame failed", error);
		} finally {
			sendingFrame = false;
		}
	}

	function clearTimer() {
		if (frameTimer !== undefined) {
			window.clearInterval(frameTimer);
			frameTimer = undefined;
		}
	}

	function stopLocal() {
		frameGeneration += 1;
		clearTimer();
		if (video) {
			video.pause();
			video.removeAttribute("src");
			video.load();
			video = undefined;
		}
		activeDevice = null;
	}

	async function startLocal(device: string) {
		if (!device.startsWith("18-") || !$settings || !$settings.screensaver_enabled) return;

		stopLocal();
		activeDevice = device;
		const generation = frameGeneration;

		try {
			if ($settings.screensaver_mode === "slideshow") {
				const photos = await Promise.all(($settings.screensaver_photo_paths ?? []).map((path) => waitForImage(path)));
				if (generation !== frameGeneration || photos.length === 0) return;

				let index = 0;
				const showPhoto = async () => {
					if (generation !== frameGeneration) return;
					const photo = photos[index];
					index = (index + 1) % photos.length;
					await sendFrame(photo, photo.naturalWidth, photo.naturalHeight, generation);
				};

				await showPhoto();
				frameTimer = window.setInterval(() => void showPhoto(), Math.max(1, $settings?.screensaver_slide_seconds || 10) * 1000);
				return;
			}

			if (!$settings.screensaver_video_path) throw new Error("No screensaver video is configured");
			const element = document.createElement("video");
			element.crossOrigin = "anonymous";
			element.muted = true;
			element.loop = true;
			element.playsInline = true;
			element.preload = "auto";
			element.src = convertFileSrc($settings.screensaver_video_path);
			video = element;
			await waitForVideo(element);
			await element.play();
			if (generation !== frameGeneration) return;

			const showVideo = () => void sendFrame(element, element.videoWidth, element.videoHeight, generation);
			showVideo();
			frameTimer = window.setInterval(showVideo, FRAME_INTERVAL_MS);
		} catch (error) {
			console.error("M18 screensaver could not start", error);
			await invoke("stop_screensaver", { device }).catch(() => undefined);
		}
	}

	onMount(() => {
		let disposed = false;
		const listeners: UnlistenFn[] = [];

		(async () => {
			const start = await listen<{ device: string }>("screensaver_start", ({ payload }) => void startLocal(payload.device));
			const stop = await listen<{ device: string }>("screensaver_stop", ({ payload }) => {
				if (payload.device === activeDevice) stopLocal();
			});
			if (disposed) {
				start();
				stop();
				return;
			}
			listeners.push(start, stop);

			const active = await invoke<string[]>("get_active_screensavers");
			for (const device of active) void startLocal(device);
		})();

		return () => {
			disposed = true;
			listeners.forEach((unlisten) => unlisten());
			stopLocal();
		};
	});
</script>
