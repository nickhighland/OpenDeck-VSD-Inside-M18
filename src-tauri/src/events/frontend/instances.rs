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

fn default_settings(action: &Action) -> serde_json::Value {
	if crate::m18::is_led_action(&action.uuid) {
		crate::m18::default_led_settings()
	} else if crate::m18_actions::is_native_action(&action.uuid) {
		crate::m18_actions::default_settings(&action.uuid)
	} else {
		serde_json::Value::Object(serde_json::Map::new())
	}
}

fn is_composite(action: &Action) -> bool {
	matches!(action.uuid.as_str(), "opendeck.multiaction" | "opendeck.toggleaction" | "opendeck.carouselaction")
}

fn lifecycle_targets(instance: &ActionInstance, targets: &mut Vec<ActionInstance>) {
	if is_composite(&instance.action) {
		for child in instance.children.iter().flatten() {
			lifecycle_targets(child, targets);
		}
	} else {
		targets.push(instance.clone());
	}
}

fn instance_contexts(instance: &ActionInstance, contexts: &mut Vec<ActionContext>) {
	contexts.push(instance.context.clone());
	for child in instance.children.iter().flatten() {
		instance_contexts(child, contexts);
	}
}

fn next_child_index(children: &[ActionInstance]) -> u16 {
	children.iter().map(|child| child.context.index).max().unwrap_or(0).saturating_add(1)
}

async fn append_child_instance(app: AppHandle, mut action: Action, parent_context: ActionContext) -> Result<Option<ActionInstance>, Error> {
	if !action.controllers.contains(&parent_context.controller) {
		return Ok(None);
	}
	if parent_context.is_root() && parent_context.controller == "Encoder" {
		let _ = crate::shared::initialise_encoder_layout(&mut action, None);
	}

	let mut locks = acquire_locks_mut().await;
	let Some(parent) = get_instance_mut(&parent_context, &mut locks).await? else {
		return Ok(None);
	};
	let Some(children) = parent.children.as_mut() else {
		return Ok(None);
	};
	let index = next_child_index(children);
	let instance = ActionInstance {
		action: action.clone(),
		context: parent.context.child(index),
		states: action.states.clone(),
		current_state: 0,
		settings: default_settings(&action),
		children: is_composite(&action).then(Vec::new),
	};
	children.push(instance.clone());

	let parent_context = parent.context.clone();
	let update_parent = matches!(parent.action.uuid.as_str(), "opendeck.toggleaction" | "opendeck.carouselaction") && parent.states.len() < children.len();
	if update_parent {
		parent.states.push(crate::shared::ActionState {
			image: parent.action.icon.clone(),
			..Default::default()
		});
	}

	if update_parent {
		let _ = update_state(&app, parent_context.clone(), &mut locks).await;
	}
	save_profile_now(&parent_context.device, &mut locks).await?;
	drop(locks);
	let _ = crate::events::outbound::will_appear::will_appear(&instance).await;

	let locks = acquire_locks().await;
	let slot = get_slot(&(&parent_context).into(), &locks).await?.clone();
	Ok(slot)
}

/// Add an action below any composite instance, including a nested composite.
/// The old `create_instance` command intentionally keeps its slot-oriented
/// API for drag/drop callers that place an action on an empty physical key.
#[command]
pub async fn create_child_instance(app: AppHandle, action: Action, parent_context: ActionContext) -> Result<Option<ActionInstance>, Error> {
	append_child_instance(app, action, parent_context).await
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
	let default_settings = default_settings(&action);

	if let Some(parent) = slot {
		let Some(children) = &mut parent.children else { return Ok(None) };
		let index = next_child_index(children);

		let instance = ActionInstance {
			action: action.clone(),
			context: parent.context.child(index),
			states: action.states.clone(),
			current_state: 0,
			settings: default_settings.clone(),
			children: is_composite(&action).then(Vec::new),
		};
		children.push(instance.clone());

		if matches!(parent.action.uuid.as_str(), "opendeck.toggleaction" | "opendeck.carouselaction") && parent.states.len() < children.len() {
			parent.states.push(crate::shared::ActionState {
				image: parent.action.icon.clone(),
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
		let instance = ActionInstance {
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
	root.join(&context.device).join(&context.profile).join(context.storage_key())
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
	let next_context = instance.context.at_context(destination.clone());
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

	let mut old_targets = Vec::new();
	lifecycle_targets(&original_source, &mut old_targets);
	lifecycle_targets(&original_destination, &mut old_targets);
	for target in old_targets {
		let _ = crate::events::outbound::will_appear::will_disappear(&target, true).await;
	}
	*get_slot_mut(&source, &mut locks).await? = Some(moved_to_source.clone());
	*get_slot_mut(&destination, &mut locks).await? = Some(moved_to_destination.clone());
	save_profile_now(&source.device, &mut locks).await?;
	drop(locks);
	let mut new_targets = Vec::new();
	lifecycle_targets(&moved_to_source, &mut new_targets);
	lifecycle_targets(&moved_to_destination, &mut new_targets);
	for target in new_targets {
		let _ = crate::events::outbound::will_appear::will_appear(&target).await;
	}
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

	let mut locks = acquire_locks_mut().await;
	if get_slot_mut(&destination, &mut locks).await?.is_some() {
		return Ok(None);
	}
	let Some(original) = get_slot_mut(&source, &mut locks).await?.clone() else {
		return Ok(None);
	};

	// Relocate exactly like a swap does: every state image of the instance and
	// of its children is copied to the destination, so custom artwork survives
	// the move (child artwork used to be reset to the action's defaults).
	let nonce = format!(
		"move-{}-{}",
		std::process::id(),
		std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos()
	);
	let moved = copy_instance_to_swap_position(&original, &destination, &nonce, &config_dir().join("images"))?;
	*get_slot_mut(&destination, &mut locks).await? = Some(moved.clone());

	if !retain {
		let mut old_targets = Vec::new();
		lifecycle_targets(&original, &mut old_targets);
		for target in old_targets {
			let _ = crate::events::outbound::will_appear::will_disappear(&target, source.profile == destination.profile).await;
		}
		let mut old_contexts = Vec::new();
		instance_contexts(&original, &mut old_contexts);
		for context in old_contexts {
			let _ = remove_dir_all(instance_images_dir(&context)).await;
		}
		*get_slot_mut(&source, &mut locks).await? = None;
		// A key moved in from another page: that page is not the selected
		// profile, so it has to be saved explicitly or the key would reappear.
		if source.profile != destination.profile {
			let device = crate::store::profiles::device_info(&source.device)?;
			locks.profile_stores.get_profile_store_mut(&device, &source.profile).await?.save()?;
			crate::key_images::forget(&source.device, Some(&source.profile), Some(source.position)).await;
		}
	}

	save_profile_now(&destination.device, &mut locks).await?;
	drop(locks);
	let mut new_targets = Vec::new();
	lifecycle_targets(&moved, &mut new_targets);
	for target in new_targets {
		let _ = crate::events::outbound::will_appear::will_appear(&target).await;
	}

	Ok(Some(moved))
}

#[command]
pub async fn remove_instance(context: ActionContext) -> Result<(), Error> {
	fn remove_child(instance: &mut ActionInstance, target: &ActionContext) -> Option<(usize, ActionInstance)> {
		let children = instance.children.as_mut()?;
		if let Some(index) = children.iter().position(|child| child.context == *target) {
			return Some((index, children.remove(index)));
		}
		children.iter_mut().find_map(|child| remove_child(child, target))
	}

	let mut locks = acquire_locks_mut().await;
	let (removed, parent_context, child_index) = {
		let slot = get_slot_mut(&(&context).into(), &mut locks).await?;
		let Some(instance) = slot else {
			return Ok(());
		};
		if instance.context == context {
			(slot.take(), None, None)
		} else {
			let Some((index, removed)) = remove_child(instance, &context) else {
				return Ok(());
			};
			(Some(removed), context.parent(), Some(index))
		}
	};

	if let (Some(parent_context), Some(index)) = (&parent_context, child_index) {
		if let Some(parent) = get_instance_mut(parent_context, &mut locks).await? {
			if parent.action.uuid == "opendeck.multiaction"
				&& let Some(settings) = parent.settings.as_object_mut()
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
			if matches!(parent.action.uuid.as_str(), "opendeck.toggleaction" | "opendeck.carouselaction") {
				let children_len = parent.children.as_ref().map(Vec::len).unwrap_or(0);
				if parent.current_state as usize >= children_len {
					parent.current_state = children_len.saturating_sub(1) as u16;
				}
				if children_len == 0 {
					parent.states.truncate(1);
				} else {
					parent.states.truncate(children_len);
				}
			}
		}
		let _ = update_state(crate::APP_HANDLE.get().unwrap(), parent_context.clone(), &mut locks).await;
	}

	save_profile_now(&context.device, &mut locks).await?;
	drop(locks);

	if let Some(removed) = removed {
		let mut targets = Vec::new();
		lifecycle_targets(&removed, &mut targets);
		for target in targets {
			let _ = crate::events::outbound::will_appear::will_disappear(&target, true).await;
		}
		let mut all = Vec::new();
		instance_contexts(&removed, &mut all);
		for context in all {
			let _ = remove_dir_all(instance_images_dir(&context)).await;
		}
	}

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
	// The appearance editor can save once more just after its key was removed.
	let Some(reference) = get_instance_mut(&context, &mut locks).await? else {
		return Ok(());
	};
	let Some(slot) = reference.states.get_mut(index as usize) else {
		return Err(anyhow::anyhow!("State {} does not exist on this key", index + 1).into());
	};
	*slot = state;
	let clone = reference.clone();
	save_profile_now(&context.device, &mut locks).await?;
	// The core draws first so that the editor's redraw, which adds the title,
	// is the one left on the M18.
	if crate::m18_actions::is_native_action(&clone.action.uuid) {
		let _ = crate::m18_actions::render(&clone).await;
	}
	// Let the key redraw itself (in the editor and on the M18) while its
	// appearance is edited in the inspector.
	let _ = update_state(crate::APP_HANDLE.get().unwrap(), context, &mut locks).await;
	drop(locks);
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
		match_switch_states(instance, count);
		instance.current_state = instance.settings.get("index").and_then(serde_json::Value::as_u64).unwrap_or(0).min((count - 1) as u64) as u16;
	}
	let clone = instance.clone();
	save_profile_now(&context.device, &mut locks).await?;
	if crate::m18_actions::is_native_action(&clone.action.uuid) {
		let _ = crate::m18_actions::render(&clone).await;
	}
	let _ = update_state(crate::APP_HANDLE.get().unwrap(), context, &mut locks).await;
	Ok(())
}

/// Give a switch action exactly one state (image and title) per shortcut.
/// New shortcuts start from the library's artwork rather than a copy of
/// another shortcut's image.
fn match_switch_states(instance: &mut ActionInstance, count: usize) {
	for states in [&mut instance.states, &mut instance.action.states] {
		states.truncate(count);
		while states.len() < count {
			let index = states.len();
			let fresh = crate::action_library::default_state(&instance.action.uuid, index).unwrap_or_default();
			states.push(fresh);
		}
	}
}

/// Remove shortcut `index` from a switch action together with its image and
/// title, keeping the next press on the same shortcut while it exists.
fn remove_shortcut(instance: &mut ActionInstance, index: usize) -> Result<(), anyhow::Error> {
	if !crate::m18_actions::is_switch_action(&instance.action.uuid) {
		return Err(anyhow::anyhow!("{} has no shortcuts to remove", instance.action.name));
	}
	let Some(hotkeys) = instance.settings.get_mut("hotkeys").and_then(serde_json::Value::as_array_mut) else {
		return Err(anyhow::anyhow!("This key has no shortcuts"));
	};
	if hotkeys.len() < 2 || index >= hotkeys.len() {
		return Err(anyhow::anyhow!("A switch keeps at least one shortcut"));
	}
	hotkeys.remove(index);
	let count = hotkeys.len();

	// Saved images are named after their state's position, so the states that
	// move up carry their image inline until the profile is saved under the
	// new positions; otherwise a later image could overwrite theirs.
	for state in instance.states.iter_mut().skip(index + 1) {
		if std::path::Path::new(&state.image).is_absolute()
			&& let Some(image) = crate::m18_actions::image_data_url(&state.image)
		{
			state.image = image;
		}
	}
	for states in [&mut instance.states, &mut instance.action.states] {
		if index < states.len() {
			states.remove(index);
		}
	}
	match_switch_states(instance, count);

	let next = instance.settings.get("index").and_then(serde_json::Value::as_u64).unwrap_or(0) as usize;
	let next = if index < next { next - 1 } else { next.min(count - 1) };
	instance.settings["index"] = serde_json::json!(next);
	instance.current_state = next as u16;
	Ok(())
}

/// Remove one of a switch action's shortcuts together with its image and title.
#[command]
pub async fn remove_switch_shortcut(context: ActionContext, index: usize) -> Result<Option<ActionInstance>, Error> {
	let mut locks = acquire_locks_mut().await;
	let Some(instance) = get_instance_mut(&context, &mut locks).await? else {
		return Ok(None);
	};
	remove_shortcut(instance, index)?;
	let clone = instance.clone();
	save_profile_now(&context.device, &mut locks).await?;
	if crate::m18_actions::is_native_action(&clone.action.uuid) {
		let _ = crate::m18_actions::render(&clone).await;
	}
	let _ = update_state(crate::APP_HANDLE.get().unwrap(), context, &mut locks).await;
	Ok(Some(clone))
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
	// Images of other pages are saved for when those pages are shown.
	let shown = Some(&context.profile) == crate::store::profiles::DEVICE_STORES.write().await.get_selected_profile(&context.device).ok().as_ref();
	if let Err(error) = crate::events::outbound::devices::update_editor_image(context, image, shown).await {
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

#[cfg(test)]
mod switch_tests {
	use super::*;

	fn switch(texts: &[&str], next: usize) -> ActionInstance {
		let categories = crate::action_library::categories();
		let action = categories
			.values()
			.flat_map(|category| category.actions.iter())
			.find(|action| action.uuid == crate::m18_actions::HOTKEY_SWITCH_UUID)
			.cloned()
			.unwrap();
		let hotkeys: Vec<_> = texts.iter().map(|text| serde_json::json!({ "down": format!("[t(\"{text}\")]"), "up": "" })).collect();
		let mut instance = ActionInstance {
			action,
			context: ActionContext {
				device: "18-TEST".to_owned(),
				profile: "Default".to_owned(),
				controller: "Keypad".to_owned(),
				position: 0,
				path: vec![],
				index: 0,
			},
			states: Vec::new(),
			current_state: next as u16,
			settings: serde_json::json!({ "hotkeys": hotkeys, "index": next }),
			children: None,
		};
		match_switch_states(&mut instance, texts.len());
		for (state, text) in instance.states.iter_mut().zip(texts) {
			state.text = (*text).to_owned();
		}
		instance
	}

	fn titles(instance: &ActionInstance) -> Vec<&str> {
		instance.states.iter().map(|state| state.text.as_str()).collect()
	}

	#[test]
	fn each_shortcut_keeps_its_own_image_and_title() {
		let mut instance = switch(&["START SCRIPT", "END SCRIPT"], 0);
		assert_eq!(titles(&instance), ["START SCRIPT", "END SCRIPT"]);

		// A new shortcut starts from the library's artwork, not a copy of another.
		instance.states[0].image = "data:image/png;base64,Y3VzdG9t".to_owned();
		match_switch_states(&mut instance, 3);
		assert_eq!(instance.states.len(), 3);
		assert_eq!(instance.action.states.len(), 3);
		assert_eq!(instance.states[2].image, "opendeck/keys/hotkey-switch.svg");
		assert_eq!(instance.states[2].text, "");
	}

	#[test]
	fn removing_a_shortcut_removes_its_appearance_and_keeps_the_next_press() {
		let mut instance = switch(&["A", "B", "C"], 2);
		remove_shortcut(&mut instance, 0).unwrap();
		assert_eq!(titles(&instance), ["B", "C"]);
		assert_eq!(instance.settings["hotkeys"].as_array().unwrap().len(), 2);
		assert_eq!((instance.settings["index"].as_u64(), instance.current_state), (Some(1), 1), "still points at C");

		let mut instance = switch(&["A", "B", "C"], 2);
		remove_shortcut(&mut instance, 2).unwrap();
		assert_eq!(titles(&instance), ["A", "B"]);
		assert_eq!(instance.current_state, 1, "the removed shortcut was next, so the last one is");

		let mut instance = switch(&["A"], 0);
		assert!(remove_shortcut(&mut instance, 0).is_err(), "a switch keeps one shortcut");
	}

	#[test]
	fn images_of_moved_shortcuts_are_carried_inline() {
		let path = std::env::temp_dir().join(format!("opendeck-switch-test-{}.png", std::process::id()));
		std::fs::write(&path, b"image bytes").unwrap();
		let mut instance = switch(&["A", "B"], 0);
		instance.states[1].image = path.to_string_lossy().into_owned();
		remove_shortcut(&mut instance, 0).unwrap();
		let _ = std::fs::remove_file(&path);
		assert!(instance.states[0].image.starts_with("data:image/png;base64,"), "{}", instance.states[0].image);
	}
}
