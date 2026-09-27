use super::send_to_all_plugins;

use crate::encoder_layouts::generate_encoder_image;
use crate::plugins::info_param::DeviceInfo;

use base64::Engine;
use image::ImageFormat;
use serde::Serialize;
use std::io::Cursor;

#[derive(Serialize)]
#[allow(non_snake_case)]
struct DeviceDidConnectEvent {
	event: &'static str,
	device: String,
	deviceInfo: DeviceInfo,
}

pub async fn device_did_connect(id: &str, info: DeviceInfo) -> Result<(), anyhow::Error> {
	send_to_all_plugins(&DeviceDidConnectEvent {
		event: "deviceDidConnect",
		device: id.to_owned(),
		deviceInfo: info,
	})
	.await
}

#[derive(Serialize)]
struct DeviceDidDisconnectEvent {
	event: &'static str,
	device: String,
}

pub async fn device_did_disconnect(id: &str) -> Result<(), anyhow::Error> {
	send_to_all_plugins(&DeviceDidDisconnectEvent {
		event: "deviceDidDisconnect",
		device: id.to_owned(),
	})
	.await
}

/// Draw a key as the core renders it.
pub async fn update_image(context: crate::shared::Context, image: Option<String>) -> Result<(), anyhow::Error> {
	send_image(context, image, false).await
}

/// Draw a key with the editor's finished image, which includes its title.
pub async fn update_editor_image(context: crate::shared::Context, image: Option<String>) -> Result<(), anyhow::Error> {
	send_image(context, image, true).await
}

/// Show another page's keys together once they are ready.
pub async fn begin_page(device: &str, positions: impl IntoIterator<Item = u8>) -> Result<(), anyhow::Error> {
	crate::m18::begin_page(device, positions).await
}

async fn send_image(context: crate::shared::Context, image: Option<String>, from_editor: bool) -> Result<(), anyhow::Error> {
	if context.device.starts_with("18-") {
		let image = match (context.controller.as_str(), image) {
			("Encoder", Some(img)) => Some(to_encoder_jpeg_data_uri(&context, &img).await?),
			(_, img) => img,
		};
		crate::m18::update_image(&context.device, context.position, image, from_editor).await?;
	}

	Ok(())
}

async fn to_encoder_jpeg_data_uri(context: &crate::shared::Context, image: &str) -> Result<String, anyhow::Error> {
	let data = image.split_once(',').unwrap().1;
	let bytes = base64::engine::general_purpose::STANDARD.decode(data)?;

	let img = generate_encoder_image(context, &bytes).await?;

	let mut buf = Vec::new();
	img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Jpeg)?;
	let encoded = base64::engine::general_purpose::STANDARD.encode(&buf);

	Ok(format!("data:image/jpeg;base64,{encoded}"))
}

/// Set the brightness for all devices.
pub async fn set_brightness(brightness: u8) -> Result<(), anyhow::Error> {
	let devices = crate::shared::DEVICES.iter().map(|device| device.id.clone()).collect::<Vec<_>>();
	for device in devices {
		// A sleeping display stays off; it wakes with the new brightness.
		if !crate::device_sleep::is_sleeping(&device) {
			set_device_brightness(&device, brightness).await?;
		}
	}

	Ok(())
}

/// Apply the LED brightness from Settings to every awake M18.
pub async fn set_led_brightness(brightness: u8) -> Result<(), anyhow::Error> {
	let devices = crate::shared::DEVICES.iter().map(|device| device.id.clone()).collect::<Vec<_>>();
	for device in devices.iter().filter(|device| !crate::device_sleep::is_sleeping(device)) {
		crate::m18::set_led_brightness(device, brightness).await?;
	}
	Ok(())
}

/// Show the LED color from Settings on every M18 whose page has no LED Colors key.
pub async fn show_settings_leds() -> Result<(), anyhow::Error> {
	let devices = crate::shared::DEVICES.iter().map(|device| device.id.clone()).collect::<Vec<_>>();
	for device in devices {
		crate::m18::show_settings_leds(&device).await?;
	}
	Ok(())
}

/// Set the brightness for a specific device.
pub async fn set_device_brightness(device: &str, brightness: u8) -> Result<(), anyhow::Error> {
	if crate::device_sleep::is_device_sleeping(device) {
		return Ok(());
	}

	if device.starts_with("18-") {
		crate::m18::set_brightness(device, brightness).await?;
	}

	Ok(())
}
