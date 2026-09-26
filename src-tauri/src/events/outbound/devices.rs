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

pub async fn update_image(context: crate::shared::Context, image: Option<String>) -> Result<(), anyhow::Error> {
	if context.device.starts_with("18-") {
		let image = match (context.controller.as_str(), image) {
			("Encoder", Some(img)) => Some(to_encoder_jpeg_data_uri(&context, &img).await?),
			(_, img) => img,
		};
		crate::m18::update_image(&context.device, context.position, image).await?;
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

pub async fn clear_screen(device: String) -> Result<(), anyhow::Error> {
	if device.starts_with("18-") {
		crate::m18::clear_screen(&device).await?;
	}

	Ok(())
}

/// Set the brightness for all devices.
pub async fn set_brightness(brightness: u8) -> Result<(), anyhow::Error> {
	for device in crate::shared::DEVICES.iter() {
		set_device_brightness(&device.id, brightness).await?;
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
