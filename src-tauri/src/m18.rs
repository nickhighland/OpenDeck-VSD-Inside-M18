use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use data_url::DataUrl;
use image::{DynamicImage, load_from_memory};
use mirajazz::{
	device::{Device, DeviceQuery, list_devices},
	error::MirajazzError,
	types::{DeviceInput, HidDeviceInfo, ImageFormat, ImageMirroring, ImageMode, ImageRotation},
};
use tokio::{
	sync::{Mutex, RwLock, mpsc},
	time::{Instant, MissedTickBehavior, interval, sleep_until, timeout},
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

/// How often the watcher enumerates HID devices.
const SCAN_INTERVAL: Duration = Duration::from_secs(2);
/// A connected device must be missing from this many consecutive scans before
/// its session is torn down. A single glitchy enumeration must not disconnect it.
const MISSING_SCANS_BEFORE_DISCONNECT: u8 = 2;
/// Upper bound for a single USB operation. A stalled HID write ends the session
/// so the watcher can reconnect, instead of freezing every button forever.
const DEVICE_IO_TIMEOUT: Duration = Duration::from_secs(5);
/// Upper bound for queueing a command for the output worker.
const QUEUE_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_RECONNECT_BACKOFF: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ButtonEvent {
	Down(u8),
	Up(u8),
}

impl ButtonEvent {
	fn position(self) -> u8 {
		match self {
			ButtonEvent::Down(position) | ButtonEvent::Up(position) => position,
		}
	}
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
	/// `from_editor` marks the editor's finished image (with its title), as
	/// opposed to the core's quick preview of a key.
	SetImage {
		position: u8,
		image: DynamicImage,
		from_editor: bool,
	},
	ClearImage(u8),
	/// Another page is about to be shown; `expected` has a bit for each LCD
	/// key that will get an image.
	BeginPage {
		expected: u32,
	},
	SetBrightness(u8),
	SetLedColors(LedPalette),
}

/// Clear any temporary full-display background (for example one left behind
/// by VSD Craft) so only key images are visible. This never touches the
/// persistent boot logo stored in the device.
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
static DEVICE_BRIGHTNESS: LazyLock<RwLock<HashMap<String, u8>>> = LazyLock::new(|| RwLock::new(HashMap::new()));
static RECONNECT_BACKOFF: LazyLock<Mutex<HashMap<String, Backoff>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Failed connection attempts for one device. Retrying every scan while
/// another application owns the M18 would spam the log every two seconds.
#[derive(Clone, Copy)]
struct Backoff {
	failures: u32,
	retry_at: Instant,
}

fn reconnect_delay(failures: u32) -> Duration {
	SCAN_INTERVAL.saturating_mul(1u32 << failures.min(5)).min(MAX_RECONNECT_BACKOFF)
}

/// Run one USB operation with a deadline so a stalled device cannot wedge the
/// session; the caller treats the error like a disconnect and reconnects.
async fn with_deadline<T>(operation: &str, deadline: Duration, future: impl Future<Output = Result<T, MirajazzError>>) -> Result<T, anyhow::Error> {
	match timeout(deadline, future).await {
		Ok(result) => result.map_err(|error| anyhow::anyhow!("{operation} failed: {error}")),
		Err(_) => Err(anyhow::anyhow!("{operation} did not finish within {} s; the USB connection appears stalled", deadline.as_secs())),
	}
}

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
	let mut missing_scans: HashMap<String, u8> = HashMap::new();
	let mut scan_failures = 0u32;
	loop {
		match list_devices(&[QUERY]).await {
			Ok(devices) => {
				if scan_failures > 0 {
					log::info!("VSD M18 device scan recovered after {scan_failures} failed attempt(s)");
					scan_failures = 0;
				}
				let candidates = devices.into_iter().filter_map(|device| candidate(device.to_device_info())).collect::<Vec<_>>();
				let present = candidates.iter().map(|candidate| candidate.id.clone()).collect::<Vec<_>>();

				let connected = SESSIONS.read().await.keys().cloned().collect::<Vec<_>>();
				missing_scans.retain(|id, _| connected.contains(id));
				for id in connected {
					if present.contains(&id) {
						missing_scans.remove(&id);
						continue;
					}
					let misses = missing_scans.entry(id.clone()).or_default();
					*misses += 1;
					if *misses >= MISSING_SCANS_BEFORE_DISCONNECT
						&& let Some(session) = SESSIONS.read().await.get(&id)
					{
						log::info!("VSD M18 device {id} is no longer present; closing its session");
						session.token.cancel();
					}
				}

				let now = Instant::now();
				for candidate in candidates {
					if SESSIONS.read().await.contains_key(&candidate.id) {
						continue;
					}
					if RECONNECT_BACKOFF.lock().await.get(&candidate.id).is_some_and(|backoff| backoff.retry_at > now) {
						continue;
					}
					let already_connecting = CONNECTING.write().await.insert(candidate.id.clone(), ()).is_some();
					if already_connecting {
						continue;
					}

					tokio::spawn(connect_candidate(candidate));
				}
			}
			Err(error) => {
				scan_failures += 1;
				// Log the first failure and then only occasionally, so a persistent
				// HID problem cannot flood the log file every two seconds.
				if scan_failures == 1 || scan_failures.is_multiple_of(30) {
					log::warn!("VSD M18 scan failed ({scan_failures} consecutive): {error}");
				}
			}
		}

		tokio::time::sleep(SCAN_INTERVAL).await;
	}
}

async fn connect_candidate(candidate: CandidateDevice) {
	let id = candidate.id.clone();
	// A panic anywhere in the session must not leave the device marked as
	// "connecting" forever, which would prevent every future reconnect.
	let result = tokio::spawn(run_session(candidate)).await;
	let outcome = match result {
		Ok(outcome) => outcome,
		Err(error) => Err(anyhow::anyhow!("session task panicked: {error}")),
	};
	match outcome {
		Ok(()) => {
			RECONNECT_BACKOFF.lock().await.remove(&id);
		}
		Err(error) => {
			let mut backoffs = RECONNECT_BACKOFF.lock().await;
			let failures = backoffs.get(&id).map(|backoff| backoff.failures + 1).unwrap_or(1);
			let delay = reconnect_delay(failures);
			backoffs.insert(
				id.clone(),
				Backoff {
					failures,
					retry_at: Instant::now() + delay,
				},
			);
			if failures == 1 || failures.is_multiple_of(10) {
				log::error!("VSD M18 device {id} could not be opened (attempt {failures}; retrying in {} s): {error:#}", delay.as_secs());
			} else {
				log::debug!("VSD M18 device {id} could not be opened (attempt {failures}): {error:#}");
			}
		}
	}
	CONNECTING.write().await.remove(&id);
}

async fn run_session(candidate: CandidateDevice) -> Result<(), anyhow::Error> {
	let id = candidate.id.clone();
	log::info!("Connecting to built-in VSD Inside M18 device {id}");
	let device = Arc::new(with_deadline("Opening the M18", Duration::from_secs(10), Device::connect(&candidate.dev, 3, KEY_COUNT, 0)).await?);
	let brightness = crate::store::current_settings().brightness;
	let initialised = async {
		with_deadline("Setting M18 brightness", DEVICE_IO_TIMEOUT, device.set_brightness(brightness)).await?;
		with_deadline("Clearing the M18 background", DEVICE_IO_TIMEOUT, clear_background_frame(&device)).await?;
		with_deadline("Clearing M18 keys", DEVICE_IO_TIMEOUT, device.clear_all_button_images()).await?;
		with_deadline("Flushing M18 keys", DEVICE_IO_TIMEOUT, device.flush()).await
	}
	.await;
	if let Err(error) = initialised {
		let _ = timeout(DEVICE_IO_TIMEOUT, device.shutdown()).await;
		return Err(error);
	}

	let token = Arc::new(CancellationToken::new());
	let (sender, receiver) = mpsc::channel(256);
	let session = Arc::new(Session { token: token.clone(), sender });
	{
		let mut sessions = SESSIONS.write().await;
		if sessions.contains_key(&id) {
			drop(sessions);
			let _ = timeout(DEVICE_IO_TIMEOUT, device.shutdown()).await;
			return Ok(());
		}
		sessions.insert(id.clone(), session);
	}
	DEVICE_BRIGHTNESS.write().await.insert(id.clone(), brightness);

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
	// The output worker must be running before the device is registered:
	// registration renders every key, and those updates must reach the LCDs
	// instead of piling up in the queue.
	let output_token = token.clone();
	let output_device = device.clone();
	let mut output_task = tokio::spawn(async move { device_output_task(output_device, receiver, output_token).await });

	if let Err(error) = crate::events::inbound::devices::register_device("", PayloadEvent { payload: info }).await {
		token.cancel();
		let _ = output_task.await;
		SESSIONS.write().await.remove(&id);
		DEVICE_BRIGHTNESS.write().await.remove(&id);
		let _ = timeout(DEVICE_IO_TIMEOUT, device.shutdown()).await;
		return Err(error);
	}
	let _ = restore_led_colors(&id).await;
	log::info!("VSD Inside M18 device {id} is ready");

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
	DEVICE_BRIGHTNESS.write().await.remove(&candidate.id);
	let _ = timeout(DEVICE_IO_TIMEOUT, device.shutdown()).await;
	log::info!("VSD Inside M18 device {} disconnected", candidate.id);
	Ok(())
}

fn log_task_result<T: std::fmt::Debug, E: std::fmt::Display>(kind: &str, result: Result<Result<T, E>, tokio::task::JoinError>) {
	match result {
		Ok(Ok(_)) => {}
		Ok(Err(error)) => log::warn!("VSD M18 {kind} task ended: {error}"),
		Err(error) => log::warn!("VSD M18 {kind} task panicked: {error}"),
	}
}

async fn device_input_task(id: String, device: Arc<Device>, token: Arc<CancellationToken>) -> Result<(), anyhow::Error> {
	let reader = device.get_reader(|_, _| Ok(DeviceInput::NoData));
	let mut buttons = ButtonSession::new();
	let mut dispatcher = KeyDispatcher::new(id);
	loop {
		if token.is_cancelled() {
			return Ok(());
		}
		// Only HID read failures end the session. Action failures are handled
		// by the key workers and can never disconnect the device.
		let report = reader.raw_read_data(512).await?;
		let Some(event) = buttons.process_report(&report)? else { continue };
		dispatcher.dispatch(event);
	}
}

/// Routes button events to one sequential worker per key.
///
/// A key's press and release are always handled in order, but a slow action
/// (a long multi-action, an interactive screenshot, an app that takes seconds
/// to launch) never blocks the HID reader or any other key.
struct KeyDispatcher {
	device: String,
	workers: HashMap<u8, mpsc::UnboundedSender<ButtonEvent>>,
}

impl KeyDispatcher {
	fn new(device: String) -> Self {
		Self { device, workers: HashMap::new() }
	}

	fn dispatch(&mut self, event: ButtonEvent) {
		let position = event.position();
		if let Some(worker) = self.workers.get(&position)
			&& worker.send(event).is_ok()
		{
			return;
		}
		// No worker yet, or the previous one died (for example after a panic in
		// an action): start a fresh one so the key keeps working.
		let (sender, receiver) = mpsc::unbounded_channel();
		tokio::spawn(key_worker(self.device.clone(), position, receiver));
		let _ = sender.send(event);
		self.workers.insert(position, sender);
	}
}

async fn key_worker(device: String, position: u8, mut events: mpsc::UnboundedReceiver<ButtonEvent>) {
	while let Some(event) = events.recv().await {
		let payload = PayloadEvent {
			payload: PressPayload { device: device.clone(), position },
		};
		let (phase, result) = match event {
			ButtonEvent::Down(_) => ("press", crate::events::inbound::devices::key_down(payload).await),
			ButtonEvent::Up(_) => ("release", crate::events::inbound::devices::key_up(payload).await),
		};
		if let Err(error) = result {
			log::warn!("M18 button {} {phase} failed: {error:#}", position + 1);
		}
	}
}

enum OutputAction {
	Command(Option<DeviceCommand>),
	Flush,
	FinishPage,
	KeepAlive,
}

/// The longest a page turn waits for the editor's images before showing
/// what it has; normally every key is ready well before this.
const PAGE_TURN_LIMIT: Duration = Duration::from_millis(600);
/// Once every key has its image, a short pause catches any last redraws.
const PAGE_TURN_SETTLE: Duration = Duration::from_millis(30);

/// A page turn in progress. Key images are collected without being shown and
/// then appear together, instead of the old page being blanked and the new
/// one filling in over several updates.
struct PageTurn {
	started: Instant,
	/// Keys on the new page that get an image.
	expected: u32,
	/// Keys that have the editor's finished image.
	finished: u32,
	/// Keys that have any image yet.
	drawn: u32,
	last_image: Instant,
}

impl PageTurn {
	fn new(expected: u32) -> Self {
		let now = Instant::now();
		Self {
			started: now,
			expected,
			finished: 0,
			drawn: 0,
			last_image: now,
		}
	}

	fn show_at(&self) -> Instant {
		let limit = self.started + PAGE_TURN_LIMIT;
		if self.expected & !self.finished == 0 {
			(self.last_image + PAGE_TURN_SETTLE).min(limit)
		} else {
			limit
		}
	}
}

enum OutputStep {
	Continue,
	ScheduleFlush,
	ClearFlush,
}

async fn device_output_task(device: Arc<Device>, mut receiver: mpsc::Receiver<DeviceCommand>, token: Arc<CancellationToken>) -> Result<(), anyhow::Error> {
	let mut keepalive = interval(Duration::from_secs(10));
	keepalive.set_missed_tick_behavior(MissedTickBehavior::Skip);
	keepalive.tick().await;

	let mut flush_deadline: Option<Instant> = None;
	let mut page_turn: Option<PageTurn> = None;
	loop {
		let far = Instant::now() + Duration::from_secs(24 * 60 * 60);
		let flush_at = flush_deadline.unwrap_or(far);
		let page_at = page_turn.as_ref().map_or(far, PageTurn::show_at);
		let action = tokio::select! {
			biased;
			_ = token.cancelled() => return Ok(()),
			_ = sleep_until(page_at), if page_turn.is_some() => OutputAction::FinishPage,
			_ = sleep_until(flush_at), if flush_deadline.is_some() => OutputAction::Flush,
			command = receiver.recv() => OutputAction::Command(command),
			_ = keepalive.tick() => OutputAction::KeepAlive,
		};

		// Every USB operation has a deadline: if the M18 stops acknowledging
		// writes, the session ends and the watcher reconnects it, instead of the
		// output queue silently filling up and every caller hanging.
		let result = match action {
			OutputAction::Command(Some(DeviceCommand::SetImage { position, image, from_editor })) => match device_key(position) {
				Some(device_position) => {
					let prepared = with_deadline("Preparing a key image", DEVICE_IO_TIMEOUT, device.set_button_image(device_position, IMAGE_FORMAT, image)).await;
					match (prepared, page_turn.as_mut()) {
						// During a page turn the image waits to be shown with the rest.
						(Ok(()), Some(turn)) => {
							let bit = 1u32 << position;
							turn.drawn |= bit;
							if from_editor {
								turn.finished |= bit;
							}
							turn.last_image = Instant::now();
							Ok(OutputStep::Continue)
						}
						(result, _) => result.map(|_| OutputStep::ScheduleFlush),
					}
				}
				None => Ok(OutputStep::Continue),
			},
			// Keys left without an image are cleared when a page turn finishes,
			// so a passing clear (say, from a key between two pages) is ignored.
			OutputAction::Command(Some(DeviceCommand::ClearImage(_))) if page_turn.is_some() => Ok(OutputStep::Continue),
			OutputAction::Command(Some(DeviceCommand::ClearImage(position))) => match device_key(position) {
				Some(device_position) => with_deadline("Clearing a key image", DEVICE_IO_TIMEOUT, device.clear_button_image(device_position))
					.await
					.map(|_| OutputStep::ScheduleFlush),
				None => Ok(OutputStep::Continue),
			},
			OutputAction::Command(Some(DeviceCommand::BeginPage { expected })) => {
				// A quick second page turn starts over; nothing is shown in between.
				page_turn = Some(PageTurn::new(expected));
				Ok(OutputStep::ClearFlush)
			}
			OutputAction::FinishPage => {
				let drawn = page_turn.take().map_or(0, |turn| turn.drawn);
				let show = async {
					if drawn == 0 {
						return device.clear_all_button_images().await;
					}
					// Keys that are empty on the new page are cleared in the same
					// update that shows the others.
					for position in (0..LCD_KEY_COUNT).filter(|position| drawn & (1u32 << position) == 0) {
						if let Some(device_position) = device_key(position) {
							device.clear_button_image(device_position).await?;
						}
					}
					device.flush().await
				};
				with_deadline("Showing a page", DEVICE_IO_TIMEOUT, show).await.map(|_| OutputStep::ClearFlush)
			}
			OutputAction::Command(Some(DeviceCommand::SetBrightness(brightness))) => with_deadline("Setting brightness", DEVICE_IO_TIMEOUT, device.set_brightness(brightness))
				.await
				.map(|_| OutputStep::Continue),
			OutputAction::Command(Some(DeviceCommand::SetLedColors(colors))) => with_deadline("Setting LED colors", DEVICE_IO_TIMEOUT, device.set_led_colors(&colors))
				.await
				.map(|_| OutputStep::Continue),
			OutputAction::Command(None) => return Ok(()),
			OutputAction::Flush => with_deadline("Updating key images", DEVICE_IO_TIMEOUT, device.flush()).await.map(|_| OutputStep::ClearFlush),
			OutputAction::KeepAlive => with_deadline("Keep-alive", DEVICE_IO_TIMEOUT, device.keep_alive()).await.map(|_| OutputStep::Continue),
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

async fn send(device: &str, command: DeviceCommand) -> Result<(), anyhow::Error> {
	let session = SESSIONS
		.read()
		.await
		.get(device)
		.cloned()
		.ok_or_else(|| anyhow::anyhow!("VSD Inside M18 device is not connected: {device}"))?;
	match timeout(QUEUE_TIMEOUT, session.sender.send(command)).await {
		Ok(Ok(())) => Ok(()),
		Ok(Err(_)) => Err(anyhow::anyhow!("VSD Inside M18 output worker is unavailable")),
		Err(_) => Err(anyhow::anyhow!("VSD Inside M18 output queue is full; the device is not accepting data")),
	}
}

pub async fn update_image(device: &str, position: u8, image: Option<String>, from_editor: bool) -> Result<(), anyhow::Error> {
	if !is_m18(device) {
		return Ok(());
	}

	match image {
		Some(image) => {
			let url = DataUrl::process(&image).map_err(|error| anyhow::anyhow!("Invalid M18 image data: {error}"))?;
			let (body, _) = url.decode_to_vec().map_err(|error| anyhow::anyhow!("Invalid M18 image data: {error}"))?;
			let decoded = load_from_memory(&body)?;
			send(
				device,
				DeviceCommand::SetImage {
					position,
					image: decoded,
					from_editor,
				},
			)
			.await
		}
		None => send(device, DeviceCommand::ClearImage(position)).await,
	}
}

/// Show the next page's keys together once they are ready. `positions` are
/// the keys that will get an image; every other key is cleared at the same
/// moment.
pub async fn begin_page(device: &str, positions: impl IntoIterator<Item = u8>) -> Result<(), anyhow::Error> {
	if is_m18(device) {
		let expected = positions
			.into_iter()
			.filter(|position| device_key(*position).is_some())
			.fold(0u32, |mask, position| mask | 1 << position);
		send(device, DeviceCommand::BeginPage { expected }).await?;
	}
	Ok(())
}

pub async fn set_brightness(device: &str, brightness: u8) -> Result<(), anyhow::Error> {
	if is_m18(device) {
		let brightness = brightness.min(100);
		let mut brightnesses = DEVICE_BRIGHTNESS.write().await;
		send(device, DeviceCommand::SetBrightness(brightness)).await?;
		brightnesses.insert(device.to_owned(), brightness);
	}
	Ok(())
}

pub async fn adjust_brightness(device: &str, adjustment: i8) -> Result<(), anyhow::Error> {
	if !is_m18(device) {
		return Ok(());
	}
	let mut brightnesses = DEVICE_BRIGHTNESS.write().await;
	let current = brightnesses.get(device).copied().unwrap_or_else(|| crate::store::current_settings().brightness);
	let brightness = adjusted_brightness(current, adjustment);
	send(device, DeviceCommand::SetBrightness(brightness)).await?;
	brightnesses.insert(device.to_owned(), brightness);
	Ok(())
}

fn adjusted_brightness(current: u8, adjustment: i8) -> u8 {
	(i16::from(current) + i16::from(adjustment)).clamp(0, 100) as u8
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
	fn a_page_turn_waits_for_the_editors_images_but_not_forever() {
		// Keys 0 and 3 get images on the new page.
		let mut turn = PageTurn::new(0b1001);
		assert_eq!(turn.show_at(), turn.started + PAGE_TURN_LIMIT, "nothing is ready yet");

		// The core's quick preview does not finish a key; the editor's image does.
		turn.drawn |= 0b1001;
		assert_eq!(turn.show_at(), turn.started + PAGE_TURN_LIMIT);
		turn.finished |= 0b0001;
		assert_eq!(turn.show_at(), turn.started + PAGE_TURN_LIMIT, "key 3 is still missing");

		turn.finished |= 0b1000;
		turn.last_image = turn.started + Duration::from_millis(120);
		assert_eq!(turn.show_at(), turn.last_image + PAGE_TURN_SETTLE, "shown right after the last image");

		// An empty page is shown straight away.
		let empty = PageTurn::new(0);
		assert_eq!(empty.show_at(), empty.last_image + PAGE_TURN_SETTLE);
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
	fn device_brightness_adjustments_are_relative_and_clamped() {
		assert_eq!(adjusted_brightness(50, -6), 44);
		assert_eq!(adjusted_brightness(44, -6), 38);
		assert_eq!(adjusted_brightness(98, 6), 100);
		assert_eq!(adjusted_brightness(3, -6), 0);
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
}
