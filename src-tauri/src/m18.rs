use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use data_url::DataUrl;
use image::{DynamicImage, GenericImageView, load_from_memory};
use mirajazz::{
	device::{Device, DeviceQuery, list_devices},
	error::MirajazzError,
	types::{DeviceInput, HidDeviceInfo, ImageFormat, ImageMirroring, ImageMode, ImageRotation},
};
use tokio::{
	sync::{RwLock, mpsc, oneshot},
	time::{Instant, MissedTickBehavior, interval, sleep_until},
};
use tokio_util::sync::CancellationToken;

use crate::events::inbound::{PayloadEvent, devices::PressPayload};

pub const DEVICE_NAMESPACE: &str = "18";
pub const LEGACY_HARDWARE_PLUGIN_ID: &str = "com.github.ibanks42.opendeck-m18.sdPlugin";
pub const LED_ACTION_UUID: &str = "opendeck.m18.led-colors";
pub const LEGACY_LED_ACTION_UUID: &str = "com.github.ibanks42.opendeck-m18.set-led-colors";
pub const VSDINSIDE_VID: u16 = 0x5548;
pub const VSDINSIDE_M18_PID: u16 = 0x1000;
pub const ROW_COUNT: usize = 4;
pub const COL_COUNT: usize = 5;
pub const KEY_COUNT: usize = ROW_COUNT * COL_COUNT;
pub const LCD_KEY_COUNT: u8 = 15;
pub const LED_COUNT: usize = 24;
const SCREEN_WIDTH: u16 = 480;
const SCREEN_HEIGHT: u16 = 272;

const QUERY: DeviceQuery = DeviceQuery::new(65440, 1, VSDINSIDE_VID, VSDINSIDE_M18_PID);
const IMAGE_FORMAT: ImageFormat = ImageFormat {
	mode: ImageMode::JPEG,
	size: (64, 64),
	rotation: ImageRotation::Rot180,
	mirror: ImageMirroring::Both,
};

pub type LedPalette = [[u8; 3]; LED_COUNT];
const DEFAULT_LED_COLOR: [u8; 3] = [0x78, 0x00, 0x00];
pub const DEFAULT_LED_PALETTE: LedPalette = [DEFAULT_LED_COLOR; LED_COUNT];

const BTN_LEFT: u8 = 0x25;
const BTN_MIDDLE: u8 = 0x30;
const BTN_RIGHT: u8 = 0x31;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ButtonEvent {
	Down(u8),
	Up(u8),
}

struct ButtonSession {
	states: [bool; KEY_COUNT],
}

impl ButtonSession {
	fn new() -> Self {
		Self { states: [false; KEY_COUNT] }
	}

	fn process_report(&mut self, report: &[u8]) -> Result<Option<ButtonEvent>, MirajazzError> {
		if !report.starts_with(&[65, 67, 75]) {
			return Ok(None);
		}

		let input = *report.get(9).ok_or(MirajazzError::BadData)?;
		let state = *report.get(10).ok_or(MirajazzError::BadData)?;
		if input == 0 {
			return Ok(None);
		}

		let position = match input {
			1..=15 => input - 1,
			BTN_LEFT => 15,
			BTN_MIDDLE => 16,
			BTN_RIGHT => 17,
			_ => return Ok(None),
		};
		let pressed = state != 0;
		let current = &mut self.states[position as usize];
		if *current == pressed {
			return Ok(None);
		}

		*current = pressed;
		Ok(Some(if pressed { ButtonEvent::Down(position) } else { ButtonEvent::Up(position) }))
	}
}

/// The hardware is vertically flipped relative to OpenDeck's logical LCD rows.
fn flip_row(key: u8) -> u8 {
	let row = key / 5;
	let column = key % 5;
	(2 - row) * 5 + column
}

fn device_key(position: u8) -> Option<u8> {
	(position < LCD_KEY_COUNT).then(|| flip_row(position))
}

#[derive(Debug, Clone)]
struct CandidateDevice {
	id: String,
	dev: HidDeviceInfo,
}

pub enum DeviceCommand {
	SetImage {
		position: u8,
		image: DynamicImage,
	},
	ScreensaverFrame {
		background: Vec<u8>,
		images: Vec<DynamicImage>,
		done: oneshot::Sender<Result<(), String>>,
	},
	ClearBackgroundFrame {
		done: oneshot::Sender<Result<(), String>>,
	},
	ClearImage(u8),
	ClearAll,
	SetBrightness(u8),
	SetLedColors(LedPalette),
}

fn background_frame_header(image_length: usize) -> Result<Vec<u8>, anyhow::Error> {
	let image_length = u32::try_from(image_length)?;
	let mut header = vec![0, b'C', b'R', b'T', 0, 0, b'B', b'G', b'P', b'I', b'C'];
	header.extend_from_slice(&image_length.to_be_bytes());
	header.extend_from_slice(&SCREEN_WIDTH.to_be_bytes());
	header.extend_from_slice(&SCREEN_HEIGHT.to_be_bytes());
	header.extend_from_slice(&0u16.to_be_bytes()); // x
	header.extend_from_slice(&0u16.to_be_bytes()); // y
	header.extend_from_slice(&[0, 0]); // reserved, layer
	Ok(header)
}

async fn write_background_frame(device: &Device, jpeg: &[u8]) -> Result<(), MirajazzError> {
	// BGPIC is the M18 SDK's *temporary* full-display frame command. This does
	// not modify the persistent boot logo stored in the device.
	let mut header = background_frame_header(jpeg.len()).map_err(|_| MirajazzError::BadData)?;
	device.write_extended_data(&mut header).await?;
	for chunk in jpeg.chunks(1024) {
		let mut packet = Vec::with_capacity(1025);
		packet.push(0);
		packet.extend_from_slice(chunk);
		device.write_extended_data(&mut packet).await?;
	}
	Ok(())
}

async fn clear_background_frame(device: &Device) -> Result<(), MirajazzError> {
	// The manufacturer's clearBackgroundFrameStream defaults to position 0x03.
	let mut header = vec![0, b'C', b'R', b'T', 0, 0, b'B', b'G', b'C', b'L', b'E', 0x03];
	device.write_extended_data(&mut header).await
}

struct Session {
	token: Arc<CancellationToken>,
	sender: mpsc::Sender<DeviceCommand>,
}

static SESSIONS: LazyLock<RwLock<HashMap<String, Arc<Session>>>> = LazyLock::new(|| RwLock::new(HashMap::new()));
static CONNECTING: LazyLock<RwLock<HashMap<String, ()>>> = LazyLock::new(|| RwLock::new(HashMap::new()));
static LED_PALETTES: LazyLock<RwLock<HashMap<String, LedPalette>>> = LazyLock::new(|| RwLock::new(HashMap::new()));

fn get_device_id(device: &HidDeviceInfo) -> Option<String> {
	Some(format!("{DEVICE_NAMESPACE}-{}", device.serial_number.clone()?))
}

fn candidate(device: HidDeviceInfo) -> Option<CandidateDevice> {
	Some(CandidateDevice {
		id: get_device_id(&device)?,
		dev: device,
	})
}

pub fn is_m18(device: &str) -> bool {
	device.starts_with("18-")
}

pub fn is_led_action(uuid: &str) -> bool {
	matches!(uuid, LED_ACTION_UUID | LEGACY_LED_ACTION_UUID)
}

pub fn default_led_settings() -> serde_json::Value {
	led_settings(&DEFAULT_LED_PALETTE)
}

pub fn led_settings(colors: &LedPalette) -> serde_json::Value {
	serde_json::json!({
		"ledColors": colors.iter().map(|[red, green, blue]| format!("#{red:02x}{green:02x}{blue:02x}")).collect::<Vec<_>>(),
	})
}

pub fn parse_led_palette(settings: &serde_json::Value) -> Option<LedPalette> {
	let colors = settings.get("ledColors")?.as_array()?;
	if colors.len() != LED_COUNT {
		return None;
	}

	let mut palette = [[0; 3]; LED_COUNT];
	for (destination, color) in palette.iter_mut().zip(colors) {
		let color = color.as_str()?.strip_prefix('#')?;
		if color.len() != 6 || !color.bytes().all(|byte| byte.is_ascii_hexdigit()) {
			return None;
		}
		*destination = [
			u8::from_str_radix(&color[0..2], 16).ok()?,
			u8::from_str_radix(&color[2..4], 16).ok()?,
			u8::from_str_radix(&color[4..6], 16).ok()?,
		];
	}

	Some(palette)
}

pub fn init() {
	tokio::spawn(watcher_task());
}

async fn watcher_task() {
	loop {
		match list_devices(&[QUERY]).await {
			Ok(devices) => {
				let candidates = devices.into_iter().filter_map(|device| candidate(device.to_device_info())).collect::<Vec<_>>();
				let present = candidates.iter().map(|candidate| candidate.id.clone()).collect::<Vec<_>>();

				let stale = SESSIONS.read().await.keys().filter(|id| !present.contains(id)).cloned().collect::<Vec<_>>();
				for id in stale {
					if let Some(session) = SESSIONS.read().await.get(&id) {
						session.token.cancel();
					}
				}

				for candidate in candidates {
					let connected = SESSIONS.read().await.contains_key(&candidate.id);
					if connected {
						continue;
					}
					let already_connecting = CONNECTING.write().await.insert(candidate.id.clone(), ()).is_some();
					if already_connecting {
						continue;
					}

					tokio::spawn(connect_candidate(candidate));
				}
			}
			Err(error) => log::warn!("VSD M18 scan failed: {error}"),
		}

		tokio::time::sleep(Duration::from_secs(2)).await;
	}
}

async fn connect_candidate(candidate: CandidateDevice) {
	let id = candidate.id.clone();
	let result = run_session(candidate).await;
	if let Err(error) = result {
		log::error!("VSD M18 session {id} ended: {error}");
	}
	CONNECTING.write().await.remove(&id);
}

async fn run_session(candidate: CandidateDevice) -> Result<(), anyhow::Error> {
	let id = candidate.id.clone();
	log::info!("Connecting to built-in VSD Inside M18 device {id}");
	let device = Arc::new(Device::connect(&candidate.dev, 3, KEY_COUNT, 0).await?);
	let brightness = crate::store::get_settings().value.brightness;
	device.set_brightness(brightness).await?;
	clear_background_frame(&device).await?;
	device.clear_all_button_images().await?;
	device.flush().await?;

	let token = Arc::new(CancellationToken::new());
	let (sender, receiver) = mpsc::channel(256);
	let session = Arc::new(Session { token: token.clone(), sender });
	if SESSIONS.write().await.insert(id.clone(), session).is_some() {
		device.shutdown().await.ok();
		return Ok(());
	}

	let info = crate::shared::DeviceInfo {
		id: id.clone(),
		plugin: String::new(),
		name: "VSD Inside M18".to_owned(),
		rows: ROW_COUNT as u8,
		columns: COL_COUNT as u8,
		encoders: 0,
		touchpoints: 0,
		infobars: 0,
		r#type: 0,
	};
	if let Err(error) = crate::events::inbound::devices::register_device("", PayloadEvent { payload: info }).await {
		SESSIONS.write().await.remove(&id);
		device.shutdown().await.ok();
		return Err(error);
	}
	let _ = restore_led_colors(&id).await;

	let output_token = token.clone();
	let output_device = device.clone();
	let mut output_task = tokio::spawn(async move { device_output_task(output_device, receiver, output_token).await });
	let input_token = token.clone();
	let input_device = device.clone();
	let mut input_task = tokio::spawn(async move { device_input_task(id.clone(), input_device, input_token).await });

	tokio::select! {
		result = &mut output_task => log_task_result("output", result),
		result = &mut input_task => log_task_result("input", result),
		_ = token.cancelled() => {},
	}
	token.cancel();
	let _ = output_task.await;
	input_task.abort();
	let _ = input_task.await;

	let _ = crate::events::inbound::devices::deregister_device("", PayloadEvent { payload: candidate.id.clone() }).await;
	SESSIONS.write().await.remove(&candidate.id);
	device.shutdown().await.ok();
	Ok(())
}

fn log_task_result<T: std::fmt::Debug, E: std::fmt::Debug>(kind: &str, result: Result<Result<T, E>, tokio::task::JoinError>) {
	match result {
		Ok(Ok(_)) => {}
		Ok(Err(error)) => log::warn!("VSD M18 {kind} task ended: {error:?}"),
		Err(error) => log::warn!("VSD M18 {kind} task panicked: {error}"),
	}
}

async fn device_input_task(id: String, device: Arc<Device>, token: Arc<CancellationToken>) -> Result<(), anyhow::Error> {
	let reader = device.get_reader(|_, _| Ok(DeviceInput::NoData));
	let mut buttons = ButtonSession::new();
	loop {
		if token.is_cancelled() {
			return Ok(());
		}
		let report = reader.raw_read_data(512).await?;
		let Some(event) = buttons.process_report(&report)? else { continue };
		let payload = PressPayload {
			device: id.clone(),
			position: match event {
				ButtonEvent::Down(position) | ButtonEvent::Up(position) => position,
			},
		};
		let result = match event {
			ButtonEvent::Down(_) => crate::events::inbound::devices::key_down(PayloadEvent { payload }).await,
			ButtonEvent::Up(_) => crate::events::inbound::devices::key_up(PayloadEvent { payload }).await,
		};
		result?;
	}
}

enum OutputAction {
	Command(Option<DeviceCommand>),
	Flush,
	KeepAlive,
}

enum OutputStep {
	Continue,
	ScheduleFlush,
	ClearFlush,
}

async fn device_output_task(device: Arc<Device>, mut receiver: mpsc::Receiver<DeviceCommand>, token: Arc<CancellationToken>) -> Result<(), MirajazzError> {
	let mut keepalive = interval(Duration::from_secs(10));
	keepalive.set_missed_tick_behavior(MissedTickBehavior::Skip);
	keepalive.tick().await;

	let mut flush_deadline: Option<Instant> = None;
	loop {
		let flush_at = flush_deadline.unwrap_or_else(|| Instant::now() + Duration::from_secs(24 * 60 * 60));
		let action = tokio::select! {
			biased;
			_ = token.cancelled() => return Ok(()),
			_ = sleep_until(flush_at), if flush_deadline.is_some() => OutputAction::Flush,
			command = receiver.recv() => OutputAction::Command(command),
			_ = keepalive.tick() => OutputAction::KeepAlive,
		};

		let result = match action {
			OutputAction::Command(Some(DeviceCommand::ScreensaverFrame { background, images, done })) => {
				let result = async {
					// Keep normal key-image updates out of the middle of a background
					// frame stream, then commit all 15 crops as one coherent frame.
					device.flush().await?;
					write_background_frame(&device, &background).await?;
					for (position, image) in images.into_iter().enumerate() {
						if let Some(key) = device_key(position as u8) {
							device.set_button_image(key, IMAGE_FORMAT, image).await?;
						}
					}
					device.flush().await?;
					Ok(OutputStep::ClearFlush)
				}
				.await;
				let _ = done.send(result.as_ref().map(|_| ()).map_err(ToString::to_string));
				result
			}
			OutputAction::Command(Some(DeviceCommand::ClearBackgroundFrame { done })) => {
				let result = clear_background_frame(&device).await.map(|_| OutputStep::Continue);
				let _ = done.send(result.as_ref().map(|_| ()).map_err(ToString::to_string));
				result
			}
			OutputAction::Command(Some(DeviceCommand::SetImage { position, image })) => {
				if let Some(device_position) = device_key(position) {
					device.set_button_image(device_position, IMAGE_FORMAT, image).await.map(|_| OutputStep::ScheduleFlush)
				} else {
					Ok(OutputStep::Continue)
				}
			}
			OutputAction::Command(Some(DeviceCommand::ClearImage(position))) => {
				if let Some(device_position) = device_key(position) {
					device.clear_button_image(device_position).await.map(|_| OutputStep::ScheduleFlush)
				} else {
					Ok(OutputStep::Continue)
				}
			}
			OutputAction::Command(Some(DeviceCommand::ClearAll)) => {
				device.clear_all_button_images().await?;
				device.flush().await?;
				Ok(OutputStep::ClearFlush)
			}
			OutputAction::Command(Some(DeviceCommand::SetBrightness(brightness))) => device.set_brightness(brightness).await.map(|_| OutputStep::Continue),
			OutputAction::Command(Some(DeviceCommand::SetLedColors(colors))) => device.set_led_colors(&colors).await.map(|_| OutputStep::Continue),
			OutputAction::Command(None) => return Ok(()),
			OutputAction::Flush => device.flush().await.map(|_| OutputStep::ClearFlush),
			OutputAction::KeepAlive => device.keep_alive().await.map(|_| OutputStep::Continue),
		};

		match result {
			Ok(OutputStep::Continue) => {}
			Ok(OutputStep::ScheduleFlush) => {
				flush_deadline.get_or_insert_with(|| Instant::now() + Duration::from_millis(50));
			}
			Ok(OutputStep::ClearFlush) => flush_deadline = None,
			Err(error) => return Err(error),
		}
	}
}

fn decode_data_image(data: &str) -> Result<(Vec<u8>, DynamicImage), anyhow::Error> {
	let url = DataUrl::process(data).map_err(|error| anyhow::anyhow!("Invalid M18 image data: {error}"))?;
	let (body, _) = url.decode_to_vec().map_err(|error| anyhow::anyhow!("Invalid M18 image data: {error}"))?;
	let decoded = load_from_memory(&body)?;
	Ok((body, decoded))
}

pub async fn set_screensaver_frame(device: &str, background: String, images: Vec<String>) -> Result<(), anyhow::Error> {
	if !is_m18(device) || images.len() != LCD_KEY_COUNT as usize {
		return Err(anyhow::anyhow!("An M18 screensaver frame needs one background and 15 key images"));
	}
	let (jpeg, decoded_background) = decode_data_image(&background)?;
	if jpeg.len() > 512 * 1024 || !jpeg.starts_with(&[0xff, 0xd8]) || decoded_background.dimensions() != (SCREEN_WIDTH.into(), SCREEN_HEIGHT.into()) {
		return Err(anyhow::anyhow!("M18 background must be a 480×272 JPEG under 512 KiB"));
	}
	let decoded_images = images
		.into_iter()
		.map(|image| {
			let (_, decoded) = decode_data_image(&image)?;
			if decoded.dimensions() != (64, 64) {
				return Err(anyhow::anyhow!("M18 key images must be 64×64"));
			}
			Ok(decoded)
		})
		.collect::<Result<Vec<_>, anyhow::Error>>()?;
	let (done, received) = oneshot::channel();
	send(
		device,
		DeviceCommand::ScreensaverFrame {
			background: jpeg,
			images: decoded_images,
			done,
		},
	)
	.await?;
	received
		.await
		.map_err(|_| anyhow::anyhow!("M18 output worker stopped during screensaver frame"))?
		.map_err(anyhow::Error::msg)
}

pub async fn clear_screensaver_background(device: &str) -> Result<(), anyhow::Error> {
	if !is_m18(device) {
		return Ok(());
	}
	let (done, received) = oneshot::channel();
	send(device, DeviceCommand::ClearBackgroundFrame { done }).await?;
	received
		.await
		.map_err(|_| anyhow::anyhow!("M18 output worker stopped while clearing background"))?
		.map_err(anyhow::Error::msg)
}

async fn send(device: &str, command: DeviceCommand) -> Result<(), anyhow::Error> {
	let session = SESSIONS
		.read()
		.await
		.get(device)
		.cloned()
		.ok_or_else(|| anyhow::anyhow!("VSD Inside M18 device is not connected: {device}"))?;
	session.sender.send(command).await.map_err(|_| anyhow::anyhow!("VSD Inside M18 output worker is unavailable"))
}

pub async fn update_image(device: &str, position: u8, image: Option<String>) -> Result<(), anyhow::Error> {
	if !is_m18(device) {
		return Ok(());
	}

	match image {
		Some(image) => {
			let url = DataUrl::process(&image).map_err(|error| anyhow::anyhow!("Invalid M18 image data: {error}"))?;
			let (body, _) = url.decode_to_vec().map_err(|error| anyhow::anyhow!("Invalid M18 image data: {error}"))?;
			let decoded = load_from_memory(&body)?;
			send(device, DeviceCommand::SetImage { position, image: decoded }).await
		}
		None => send(device, DeviceCommand::ClearImage(position)).await,
	}
}

pub async fn clear_screen(device: &str) -> Result<(), anyhow::Error> {
	if is_m18(device) {
		send(device, DeviceCommand::ClearAll).await?;
	}
	Ok(())
}

pub async fn set_brightness(device: &str, brightness: u8) -> Result<(), anyhow::Error> {
	if is_m18(device) {
		send(device, DeviceCommand::SetBrightness(brightness)).await?;
	}
	Ok(())
}

pub async fn set_led_colors(device: &str, colors: LedPalette) -> Result<(), anyhow::Error> {
	if is_m18(device) {
		LED_PALETTES.write().await.insert(device.to_owned(), colors);
		send(device, DeviceCommand::SetLedColors(colors)).await?;
	}
	Ok(())
}

pub async fn apply_led_action(instance: &crate::shared::ActionInstance) -> Result<(), anyhow::Error> {
	if !is_led_action(&instance.action.uuid) {
		return Ok(());
	}
	let palette = parse_led_palette(&instance.settings).unwrap_or(DEFAULT_LED_PALETTE);
	set_led_colors(&instance.context.device, palette).await
}

pub async fn restore_led_colors(device: &str) -> Result<(), anyhow::Error> {
	if let Some(colors) = LED_PALETTES.read().await.get(device).copied() {
		send(device, DeviceCommand::SetLedColors(colors)).await?;
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	fn report(input: u8, state: u8) -> [u8; 11] {
		let mut report = [0; 11];
		report[..3].copy_from_slice(&[65, 67, 75]);
		report[9] = input;
		report[10] = state;
		report
	}

	#[test]
	fn maps_m18_bottom_buttons_after_lcd_keys() {
		let mut session = ButtonSession::new();
		assert_eq!(session.process_report(&report(BTN_LEFT, 1)).unwrap(), Some(ButtonEvent::Down(15)));
		assert_eq!(session.process_report(&report(BTN_MIDDLE, 1)).unwrap(), Some(ButtonEvent::Down(16)));
		assert_eq!(session.process_report(&report(BTN_RIGHT, 1)).unwrap(), Some(ButtonEvent::Down(17)));
	}

	#[test]
	fn maps_only_lcd_positions_to_hid_image_rows() {
		assert_eq!(device_key(0), Some(10));
		assert_eq!(device_key(4), Some(14));
		assert_eq!(device_key(10), Some(0));
		assert_eq!(device_key(14), Some(4));
		assert_eq!(device_key(15), None);
		assert_eq!(device_key(17), None);
	}

	#[test]
	fn ignores_unrecognised_hid_inputs() {
		let mut session = ButtonSession::new();
		assert_eq!(session.process_report(&report(0x40, 1)).unwrap(), None);
	}

	#[test]
	fn parses_and_serializes_led_palettes() {
		let settings = default_led_settings();
		assert_eq!(parse_led_palette(&settings), Some(DEFAULT_LED_PALETTE));
		assert_eq!(led_settings(&DEFAULT_LED_PALETTE), settings);
	}

	#[test]
	fn temporary_background_header_has_full_screen_geometry() {
		let header = background_frame_header(0x1234).unwrap();
		assert_eq!(&header[..11], &[0, b'C', b'R', b'T', 0, 0, b'B', b'G', b'P', b'I', b'C']);
		assert_eq!(&header[11..15], &0x1234u32.to_be_bytes());
		assert_eq!(&header[15..19], &[0x01, 0xe0, 0x01, 0x10]);
		assert_eq!(&header[19..], &[0, 0, 0, 0, 0, 0]);
	}
}
