use super::Error;

use crate::shared::{Action, ActionContext, ActionInstance, ActionState, Context, config_dir};
use crate::store::profiles::{LocksMut, acquire_locks, acquire_locks_mut, get_instance_mut, get_slot, get_slot_mut, save_profile_now};

use tauri::{AppHandle, Emitter, command};
use tokio::fs::remove_dir_all;

#[derive(serde::Serialize)]
pub struct SwapResult {
	pub source: ActionInstance,
	pub destination: ActionInstance,
}

#[command]
pub async fn create_instance(app: AppHandle, mut action: Action, context: Context) -> Result<Option<ActionInstance>, Error> {
	if !action.controllers.contains(&context.controller) {
		return Ok(None);
	}

	if context.controller == "Encoder" {
		let _ = crate::shared::initialise_encoder_layout(&mut action, None);
	}

	let mut locks = acquire_locks_mut().await;
	let slot = get_slot_mut(&context, &mut locks).await?;
	let default_settings = if crate::m18::is_led_action(&action.uuid) {
		crate::m18::default_led_settings()
	} else if crate::m18_actions::is_native_action(&action.uuid) {
		crate::m18_actions::default_settings(&action.uuid)
	} else {
		serde_json::Value::Object(serde_json::Map::new())
	};

	if let Some(parent) = slot {
		let Some(children) = &mut parent.children else { return Ok(None) };
		let index = match children.last() {
			None => 1,
			Some(instance) => instance.context.index + 1,
		};

		let mut instance = ActionInstance {
			action: action.clone(),
			context: ActionContext::from_context(context.clone(), index),
			states: action.states.clone(),
			current_state: 0,
			settings: default_settings.clone(),
			children: None,
		};
		crate::m18_actions::refresh_open_app_icon(&mut instance);
		children.push(instance.clone());

		if matches!(parent.action.uuid.as_str(), "opendeck.toggleaction" | "opendeck.carouselaction") && parent.states.len() < children.len() {
			parent.states.push(crate::shared::ActionState {
				image: "opendeck/toggle-action.png".to_owned(),
				..Default::default()
			});
			let _ = update_state(&app, parent.context.clone(), &mut locks).await;
		}

		save_profile_now(&context.device, &mut locks).await?;
		drop(locks);
		let _ = crate::events::outbound::will_appear::will_appear(&instance).await;

		let locks = acquire_locks().await;
		let slot = get_slot(&context, &locks).await?.clone();
		Ok(slot)
	} else {
		let mut instance = ActionInstance {
			action: action.clone(),
			context: ActionContext::from_context(context.clone(), 0),
			states: action.states.clone(),
			current_state: 0,
			settings: default_settings,
			children: if matches!(action.uuid.as_str(), "opendeck.multiaction" | "opendeck.toggleaction" | "opendeck.carouselaction") {
				Some(vec![])
			} else {
				None
			},
		};
		crate::m18_actions::refresh_open_app_icon(&mut instance);

		*slot = Some(instance.clone());
		let slot = slot.clone();

		save_profile_now(&context.device, &mut locks).await?;
		let _ = crate::events::outbound::will_appear::will_appear(&instance).await;

		Ok(slot)
	}
}

fn instance_images_dir(context: &ActionContext) -> std::path::PathBuf {
	instance_images_dir_at(&config_dir().join("images"), context)
}

fn instance_images_dir_at(root: &std::path::Path, context: &ActionContext) -> std::path::PathBuf {
	root.join(&context.device)
		.join(&context.profile)
		.join(format!("{}.{}.{}", context.controller, context.position, context.index))
}

fn copy_swap_image(image: &mut String, old_dir: &std::path::Path, new_dir: &std::path::Path, nonce: &str) -> Result<(), anyhow::Error> {
	let path = std::path::Path::new(image);
	let Ok(relative) = path.strip_prefix(old_dir) else { return Ok(()) };
	let destination = new_dir.join(nonce).join(relative);
	std::fs::create_dir_all(destination.parent().ok_or_else(|| anyhow::anyhow!("Invalid image destination"))?)?;
	std::fs::copy(path, &destination)?;
	*image = destination.to_string_lossy().into_owned();
	Ok(())
}

fn copy_instance_to_swap_position(instance: &ActionInstance, destination: &Context, nonce: &str, image_root: &std::path::Path) -> Result<ActionInstance, anyhow::Error> {
	let mut moved = instance.clone();
	let old_dir = instance_images_dir_at(image_root, &instance.context);
	let next_context = ActionContext::from_context(destination.clone(), instance.context.index);
	let new_dir = instance_images_dir_at(image_root, &next_context);
	copy_swap_image(&mut moved.action.icon, &old_dir, &new_dir, nonce)?;
	for state in &mut moved.action.states {
		copy_swap_image(&mut state.image, &old_dir, &new_dir, nonce)?;
	}
	for state in &mut moved.states {
		copy_swap_image(&mut state.image, &old_dir, &new_dir, nonce)?;
	}
	moved.context = next_context;
	if let Some(children) = &mut moved.children {
		for child in children {
			*child = copy_instance_to_swap_position(child, destination, nonce, image_root)?;
		}
	}
	Ok(moved)
}

#[command]
pub async fn swap_m18_instances(source: Context, destination: Context) -> Result<SwapResult, Error> {
	if !crate::m18::is_m18(&source.device)
		|| source.device != destination.device
		|| source.profile != destination.profile
		|| source.controller != "Keypad"
		|| destination.controller != "Keypad"
		|| source.position >= 18
		|| destination.position >= 18
		|| source.position == destination.position
	{
		return Err(anyhow::anyhow!("M18 swaps require two distinct positions on the same page").into());
	}

	let mut locks = acquire_locks_mut().await;
	let original_source = get_slot_mut(&source, &mut locks).await?.clone().ok_or_else(|| anyhow::anyhow!("Source button is empty"))?;
	let original_destination = get_slot_mut(&destination, &mut locks).await?.clone().ok_or_else(|| anyhow::anyhow!("Destination button is empty"))?;
	let nonce = format!(
		"swap-{}-{}",
		std::process::id(),
		std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos()
	);
	let image_root = config_dir().join("images");
	let moved_to_destination = copy_instance_to_swap_position(&original_source, &destination, &nonce, &image_root)?;
	let moved_to_source = copy_instance_to_swap_position(&original_destination, &source, &nonce, &image_root)?;

	let _ = crate::events::outbound::will_appear::will_disappear(&original_source, true).await;
	let _ = crate::events::outbound::will_appear::will_disappear(&original_destination, true).await;
	*get_slot_mut(&source, &mut locks).await? = Some(moved_to_source.clone());
	*get_slot_mut(&destination, &mut locks).await? = Some(moved_to_destination.clone());
	save_profile_now(&source.device, &mut locks).await?;
	drop(locks);
	let _ = crate::events::outbound::will_appear::will_appear(&moved_to_source).await;
	let _ = crate::events::outbound::will_appear::will_appear(&moved_to_destination).await;
	Ok(SwapResult {
		source: moved_to_source,
		destination: moved_to_destination,
	})
}

#[cfg(test)]
mod swap_tests {
	use super::*;

	fn instance(context: Context, label: &str, index: u16) -> ActionInstance {
		ActionInstance {
			action: serde_json::from_value(serde_json::json!({
				"name": label,
				"uuid": format!("test.{label}"),
				"states": [{ "image": "data:image/png;base64,AAAA" }]
			}))
			.unwrap(),
			context: ActionContext::from_context(context, index),
			states: vec![ActionState {
				image: format!("data:image/png;base64,{label}"),
				..Default::default()
			}],
			current_state: 0,
			settings: serde_json::json!({ "shortcut": label }),
			children: None,
		}
	}

	#[test]
	fn swapping_buttons_preserves_actions_settings_and_child_contexts() {
		let first = Context {
			device: "18-test".into(),
			profile: "page1".into(),
			controller: "Keypad".into(),
			position: 0,
		};
		let bottom = Context { position: 17, ..first.clone() };
		let mut source = instance(first.clone(), "first", 0);
		source.children = Some(vec![instance(first.clone(), "child", 1)]);
		let destination = instance(bottom.clone(), "second", 0);
		let root = std::path::Path::new("/unused-test-images");
		let moved_source = copy_instance_to_swap_position(&source, &bottom, "test", root).unwrap();
		let moved_destination = copy_instance_to_swap_position(&destination, &first, "test", root).unwrap();
		assert_eq!(moved_source.context.position, 17);
		assert_eq!(moved_source.children.as_ref().unwrap()[0].context.position, 17);
		assert_eq!(moved_source.children.as_ref().unwrap()[0].context.index, 1);
		assert_eq!(moved_source.settings["shortcut"], "first");
		assert_eq!(moved_source.states[0].image, source.states[0].image);
		assert_eq!(moved_destination.context.position, 0);
		assert_eq!(moved_destination.settings["shortcut"], "second");
	}

	#[test]
	fn swapping_copies_custom_artwork_to_the_destination_slot() {
		let root = std::env::temp_dir().join(format!(
			"opendeck-m18-swap-test-{}-{}",
			std::process::id(),
			std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos()
		));
		let first = Context {
			device: "18-test".into(),
			profile: "page1".into(),
			controller: "Keypad".into(),
			position: 0,
		};
		let second = Context { position: 15, ..first.clone() };
		let mut source = instance(first.clone(), "first", 0);
		let old_dir = instance_images_dir_at(&root, &source.context);
		std::fs::create_dir_all(&old_dir).unwrap();
		let old_image = old_dir.join("custom.png");
		std::fs::write(&old_image, b"custom artwork").unwrap();
		source.states[0].image = old_image.to_string_lossy().into_owned();

		let moved = copy_instance_to_swap_position(&source, &second, "test-copy", &root).unwrap();
		assert_eq!(std::fs::read(&moved.states[0].image).unwrap(), b"custom artwork");
		assert!(moved.states[0].image.contains("Keypad.15.0"));
		assert_eq!(std::fs::read(&old_image).unwrap(), b"custom artwork");
		std::fs::remove_dir_all(&root).unwrap();
	}
}

#[command]
pub async fn move_instance(source: Context, destination: Context, retain: bool) -> Result<Option<ActionInstance>, Error> {
	if source.controller != destination.controller {
		return Ok(None);
	}

	{
		let locks = crate::store::profiles::acquire_locks().await;
		let dst = crate::store::profiles::get_slot(&destination, &locks).await?;
		if dst.is_some() {
			return Ok(None);
		}
	}

	let mut locks = acquire_locks_mut().await;
	let src = get_slot_mut(&source, &mut locks).await?;

	let Some(mut new) = src.clone() else {
		return Ok(None);
	};
	new.context = ActionContext::from_context(destination.clone(), 0);
	if let Some(children) = &mut new.children {
		for (index, instance) in children.iter_mut().enumerate() {
			instance.context = ActionContext::from_context(destination.clone(), index as u16 + 1);
			for (i, state) in instance.states.iter_mut().enumerate() {
				if !instance.action.states[i].image.is_empty() {
					state.image = instance.action.states[i].image.clone();
				} else {
					state.image = instance.action.icon.clone();
				}
			}
		}
	}

	let old_dir = instance_images_dir(&src.as_ref().unwrap().context);
	let new_dir = instance_images_dir(&new.context);
	let _ = tokio::fs::create_dir_all(&new_dir).await;
	if let Ok(files) = old_dir.read_dir() {
		for file in files.flatten() {
			let _ = tokio::fs::copy(file.path(), new_dir.join(file.file_name())).await;
		}
	}
	for state in new.states.iter_mut() {
		let path = std::path::Path::new(&state.image);
		if path.starts_with(&old_dir) {
			state.image = new_dir.join(path.strip_prefix(&old_dir).unwrap()).to_string_lossy().into_owned();
		}
	}

	let dst = get_slot_mut(&destination, &mut locks).await?;
	*dst = Some(new.clone());

	if !retain {
		let src = get_slot_mut(&source, &mut locks).await?;
		if let Some(old) = src {
			let _ = crate::events::outbound::will_appear::will_disappear(old, true).await;
			let _ = remove_dir_all(instance_images_dir(&old.context)).await;
		}
		*src = None;
	}

	let _ = crate::events::outbound::will_appear::will_appear(&new).await;

	save_profile_now(&destination.device, &mut locks).await?;

	Ok(Some(new))
}

#[command]
pub async fn remove_instance(context: ActionContext) -> Result<(), Error> {
	let mut locks = acquire_locks_mut().await;
	let slot = get_slot_mut(&(&context).into(), &mut locks).await?;
	let Some(instance) = slot else {
		return Ok(());
	};

	if instance.context == context {
		let _ = crate::events::outbound::will_appear::will_disappear(instance, true).await;
		if let Some(children) = &instance.children {
			for child in children {
				let _ = crate::events::outbound::will_appear::will_disappear(child, true).await;
				let _ = remove_dir_all(instance_images_dir(&child.context)).await;
			}
		}
		let _ = remove_dir_all(instance_images_dir(&instance.context)).await;
		*slot = None;
	} else {
		let children = instance.children.as_mut().unwrap();
		for (index, child) in children.iter().enumerate() {
			if child.context == context {
				let _ = crate::events::outbound::will_appear::will_disappear(child, true).await;
				let _ = remove_dir_all(instance_images_dir(&child.context)).await;
				children.remove(index);

				if instance.action.uuid == "opendeck.multiaction"
					&& let Some(settings) = instance.settings.as_object_mut()
					&& let Some(delays) = settings.get_mut("delays").and_then(|v| v.as_array_mut())
				{
					if index == 0 {
						if !delays.is_empty() {
							delays.remove(0);
						}
					} else if index - 1 < delays.len() {
						delays.remove(index - 1);
					}
				}

				break;
			}
		}
		if matches!(instance.action.uuid.as_str(), "opendeck.toggleaction" | "opendeck.carouselaction") {
			if instance.current_state as usize >= children.len() {
				instance.current_state = if children.is_empty() { 0 } else { children.len() as u16 - 1 };
			}
			if !children.is_empty() {
				instance.states.pop();
				let _ = update_state(crate::APP_HANDLE.get().unwrap(), instance.context.clone(), &mut locks).await;
			}
		}
	}

	save_profile_now(&context.device, &mut locks).await?;

	Ok(())
}

#[derive(Clone, serde::Serialize)]
struct UpdateStateEvent {
	context: ActionContext,
	contents: Option<ActionInstance>,
}

pub async fn update_state(app: &AppHandle, context: ActionContext, locks: &mut LocksMut<'_>) -> Result<(), anyhow::Error> {
	app.emit(
		"update_state",
		UpdateStateEvent {
			contents: get_instance_mut(&context, locks).await?.cloned(),
			context,
		},
	)?;
	Ok(())
}

/// Flash the alert badge on a key. The editor redraws the key with the badge
/// and forwards that image to the M18, so the failure is visible on the device.
pub fn show_alert(context: &ActionContext) {
	if let Some(app) = crate::APP_HANDLE.get()
		&& let Err(error) = app.emit("show_alert", context.to_string())
	{
		log::debug!("Failed to show alert for {context}: {error}");
	}
}

#[command]
pub async fn set_state(context: ActionContext, index: u16, state: ActionState) -> Result<(), Error> {
	let mut locks = acquire_locks_mut().await;
	let reference = get_instance_mut(&context, &mut locks).await?.unwrap();
	reference.states[index as usize] = state;
	let clone = reference.clone();
	save_profile_now(&context.device, &mut locks).await?;
	if crate::m18_actions::is_native_action(&clone.action.uuid) {
		let _ = crate::m18_actions::render(&clone).await;
	}
	crate::events::outbound::states::title_parameters_did_change(&clone, index).await?;
	Ok(())
}

#[command]
pub async fn set_instance_settings(context: ActionContext, settings: serde_json::Value) -> Result<(), Error> {
	let mut locks = acquire_locks_mut().await;
	let Some(instance) = get_instance_mut(&context, &mut locks).await? else {
		return Ok(());
	};
	instance.settings = settings;
	if crate::m18_actions::is_switch_action(&instance.action.uuid) {
		let count = instance.settings.get("hotkeys").and_then(serde_json::Value::as_array).map(Vec::len).unwrap_or(0).max(1);
		let template = instance.states.first().cloned().unwrap_or_default();
		instance.states.resize(count, template.clone());
		instance.action.states.resize(count, template);
		instance.current_state = instance.settings.get("index").and_then(serde_json::Value::as_u64).unwrap_or(0).min((count - 1) as u64) as u16;
	}
	crate::m18_actions::refresh_open_app_icon(instance);
	let clone = instance.clone();
	save_profile_now(&context.device, &mut locks).await?;
	if crate::m18_actions::is_native_action(&clone.action.uuid) {
		let _ = crate::m18_actions::render(&clone).await;
	}
	let _ = update_state(crate::APP_HANDLE.get().unwrap(), context, &mut locks).await;
	Ok(())
}

#[command]
pub async fn set_child_delay(parent_context: ActionContext, index: usize, delay_ms: u64) -> Result<serde_json::Value, Error> {
	let mut locks = acquire_locks_mut().await;
	let Some(parent) = get_instance_mut(&parent_context, &mut locks).await? else {
		return Ok(serde_json::Value::Null);
	};

	let delays = parent.settings.get_mut("delays").and_then(|v| v.as_array_mut());
	if let Some(arr) = delays {
		if arr.len() <= index {
			arr.resize(index + 1, serde_json::json!(100));
		}
		arr[index] = serde_json::json!(delay_ms);
	} else {
		if !parent.settings.is_object() {
			parent.settings = serde_json::Value::Object(serde_json::Map::new());
		}
		let map = parent.settings.as_object_mut().unwrap();
		let mut arr = vec![serde_json::json!(100); index + 1];
		arr[index] = serde_json::json!(delay_ms);
		map.insert("delays".to_string(), serde_json::Value::Array(arr));
	}
	let parent_settings = parent.settings.clone();

	save_profile_now(&parent_context.device, &mut locks).await?;
	Ok(parent_settings)
}

#[command]
pub async fn set_m18_led_palette(context: ActionContext, colors: Vec<String>) -> Result<(), Error> {
	let settings = serde_json::json!({ "ledColors": colors });
	let palette = crate::m18::parse_led_palette(&settings).ok_or_else(|| Error::new("Invalid M18 LED palette".to_owned()))?;
	let mut locks = acquire_locks_mut().await;
	let Some(instance) = get_instance_mut(&context, &mut locks).await? else {
		return Ok(());
	};
	if !crate::m18::is_led_action(&instance.action.uuid) {
		return Err(Error::new("Action is not an M18 LED action".to_owned()));
	}
	instance.settings = crate::m18::led_settings(&palette);
	crate::m18::set_led_colors(&context.device, palette).await?;
	save_profile_now(&context.device, &mut locks).await?;
	let _ = update_state(crate::APP_HANDLE.get().unwrap(), context, &mut locks).await;
	Ok(())
}

#[command]
pub async fn update_image(context: Context, image: Option<String>) {
	if Some(&context.profile) != crate::store::profiles::DEVICE_STORES.write().await.get_selected_profile(&context.device).ok().as_ref() {
		return;
	}

	if let Err(error) = crate::events::outbound::devices::update_image(context, image).await {
		log::warn!("Failed to update device image: {}", error);
	}
}

#[command]
pub async fn trigger_virtual_press(context: Context) -> Result<(), Error> {
	let event = || crate::events::inbound::PayloadEvent {
		payload: crate::events::inbound::devices::PressPayload {
			device: context.device.clone(),
			position: context.position,
		},
	};
	match context.controller.as_str() {
		"Keypad" => {
			crate::events::inbound::devices::key_down(event()).await?;
			tokio::time::sleep(std::time::Duration::from_millis(100)).await;
			crate::events::inbound::devices::key_up(event()).await?;
		}
		"Encoder" => {
			crate::events::inbound::devices::encoder_down(event()).await?;
			tokio::time::sleep(std::time::Duration::from_millis(100)).await;
			crate::events::inbound::devices::encoder_up(event()).await?;
		}
		_ => {}
	}

	Ok(())
}

#[derive(Clone, serde::Serialize)]
struct KeyMovedEvent {
	context: Context,
	pressed: bool,
}

pub async fn key_moved(app: &AppHandle, context: Context, pressed: bool) -> Result<(), anyhow::Error> {
	app.emit("key_moved", KeyMovedEvent { context, pressed })?;
	Ok(())
}
