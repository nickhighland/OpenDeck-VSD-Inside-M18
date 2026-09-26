use super::{GenericInstancePayload, send_to_plugin};

use crate::events::frontend::instances::{key_moved, update_state};
use crate::shared::{ActionContext, Context};
use crate::store::profiles::{acquire_locks_mut, get_slot_mut, mark_profile_stale};

use std::sync::LazyLock;
use std::time::Duration;

use dashmap::DashMap;
use serde::Serialize;

static KEY_DOWN_TARGETS: LazyLock<DashMap<(String, u8), Context>> = LazyLock::new(DashMap::new);

#[derive(Serialize)]
struct KeyEvent {
	event: &'static str,
	action: String,
	context: ActionContext,
	device: String,
	payload: GenericInstancePayload,
}

pub async fn key_down(device: &str, key: u8) -> Result<(), anyhow::Error> {
	let mut locks = acquire_locks_mut().await;
	let selected_profile = locks.device_stores.get_selected_profile(device)?;
	let context = Context {
		device: device.to_owned(),
		profile: selected_profile.to_owned(),
		controller: "Keypad".to_owned(),
		position: key,
	};

	let _ = key_moved(crate::APP_HANDLE.get().unwrap(), context.clone(), true).await;
	KEY_DOWN_TARGETS.insert((device.to_owned(), key), context.clone());

	let Some(instance) = get_slot_mut(&context, &mut locks).await? else { return Ok(()) };
	if crate::m18::is_led_action(&instance.action.uuid) {
		crate::m18::apply_led_action(instance).await?;
		return Ok(());
	}
	if crate::m18_actions::is_native_action(&instance.action.uuid) {
		let native_instance = instance.clone();
		drop(locks);
		if let Err(error) = crate::m18_actions::key_down(&native_instance).await {
			log::warn!("native M18 key-down action failed: {error:#}");
		}
		return Ok(());
	}
	if instance.action.uuid == "opendeck.multiaction" {
		let children = instance.children.clone().unwrap_or_default();
		let delays: Vec<u64> = instance
			.settings
			.get("delays")
			.and_then(|v| v.as_array())
			.map(|arr| arr.iter().filter_map(|v| v.as_u64()).collect())
			.unwrap_or_default();

		drop(locks);

		for (i, child) in children.iter().enumerate() {
			send_to_plugin(
				&child.action.plugin,
				&KeyEvent {
					event: "keyDown",
					action: child.action.uuid.clone(),
					context: child.context.clone(),
					device: child.context.device.clone(),
					payload: GenericInstancePayload::new(child),
				},
			)
			.await?;

			tokio::time::sleep(Duration::from_millis(100)).await;

			send_to_plugin(
				&child.action.plugin,
				&KeyEvent {
					event: "keyUp",
					action: child.action.uuid.clone(),
					context: child.context.clone(),
					device: child.context.device.clone(),
					payload: GenericInstancePayload::new(child),
				},
			)
			.await?;

			let delay = delays.get(i).copied().unwrap_or(100);
			if delay > 0 {
				tokio::time::sleep(Duration::from_millis(delay)).await;
			}
		}

		let mut locks = acquire_locks_mut().await;

		if let Some(instance) = get_slot_mut(&context, &mut locks).await?
			&& let Some(children) = &mut instance.children
		{
			for child in &mut *children {
				if child.states.len() == 2 && !child.action.disable_automatic_states {
					child.current_state = (child.current_state + 1) % (child.states.len() as u16);
				}
			}

			for child in children.iter().map(|x| x.context.clone()).collect::<Vec<_>>() {
				let _ = update_state(crate::APP_HANDLE.get().unwrap(), child, &mut locks).await;
			}
		}

		mark_profile_stale(device, &mut locks).await?;
	} else if instance.action.uuid == "opendeck.toggleaction" {
		let children = instance.children.as_ref().unwrap();
		if children.is_empty() {
			return Ok(());
		}
		let child = &children[instance.current_state as usize];
		send_to_plugin(
			&child.action.plugin,
			&KeyEvent {
				event: "keyDown",
				action: child.action.uuid.clone(),
				context: child.context.clone(),
				device: child.context.device.clone(),
				payload: GenericInstancePayload::new(child),
			},
		)
		.await?;
	} else {
		send_to_plugin(
			&instance.action.plugin,
			&KeyEvent {
				event: "keyDown",
				action: instance.action.uuid.clone(),
				context: instance.context.clone(),
				device: instance.context.device.clone(),
				payload: GenericInstancePayload::new(instance),
			},
		)
		.await?;
	}

	Ok(())
}

pub async fn key_up(device: &str, key: u8) -> Result<(), anyhow::Error> {
	let mut locks = acquire_locks_mut().await;
	let selected_profile = locks.device_stores.get_selected_profile(device)?;
	let context = Context {
		device: device.to_owned(),
		profile: selected_profile.to_owned(),
		controller: "Keypad".to_owned(),
		position: key,
	};

	let _ = key_moved(crate::APP_HANDLE.get().unwrap(), context.clone(), false).await;
	let Some((_, expected_context)) = KEY_DOWN_TARGETS.remove(&(device.to_owned(), key)) else {
		return Ok(());
	};
	if context != expected_context {
		return Ok(());
	}

	let slot = get_slot_mut(&context, &mut locks).await?;
	let Some(instance) = slot else { return Ok(()) };
	if crate::m18::is_led_action(&instance.action.uuid) {
		return Ok(());
	}
	if crate::m18_actions::is_native_action(&instance.action.uuid) {
		let native_instance = instance.clone();
		let native_context: Context = (&native_instance.context).into();
		drop(locks);
		let advances_state = crate::m18_actions::key_up(&native_instance).await?;
		if advances_state {
			let mut locks = acquire_locks_mut().await;
			let updated_instance = if let Some(instance) = get_slot_mut(&native_context, &mut locks).await? {
				if instance.states.len() > 1 && !instance.action.disable_automatic_states {
					instance.current_state = (instance.current_state + 1) % (instance.states.len() as u16);
					if crate::m18_actions::is_switch_action(&instance.action.uuid) {
						instance.settings["index"] = serde_json::json!(instance.current_state);
					}
				}
				Some(instance.clone())
			} else {
				None
			};
			if let Some(instance) = updated_instance {
				let context = instance.context.clone();
				let _ = update_state(crate::APP_HANDLE.get().unwrap(), context, &mut locks).await;
				let _ = crate::m18_actions::render(&instance).await;
			}
			mark_profile_stale(device, &mut locks).await?;
		}
		return Ok(());
	}

	if instance.action.uuid == "opendeck.toggleaction" {
		let index = instance.current_state as usize;
		let children = instance.children.as_ref().unwrap();
		if children.is_empty() {
			return Ok(());
		}
		let child = &children[index];
		send_to_plugin(
			&child.action.plugin,
			&KeyEvent {
				event: "keyUp",
				action: child.action.uuid.clone(),
				context: child.context.clone(),
				device: child.context.device.clone(),
				payload: GenericInstancePayload::new(child),
			},
		)
		.await?;
		instance.current_state = ((index + 1) % instance.children.as_ref().unwrap().len()) as u16;
	} else if instance.action.uuid != "opendeck.multiaction" {
		if instance.states.len() == 2 && !instance.action.disable_automatic_states {
			instance.current_state = (instance.current_state + 1) % (instance.states.len() as u16);
		}
		send_to_plugin(
			&instance.action.plugin,
			&KeyEvent {
				event: "keyUp",
				action: instance.action.uuid.clone(),
				context: instance.context.clone(),
				device: instance.context.device.clone(),
				payload: GenericInstancePayload::new(instance),
			},
		)
		.await?;
	};

	let _ = update_state(crate::APP_HANDLE.get().unwrap(), instance.context.clone(), &mut locks).await;
	mark_profile_stale(device, &mut locks).await?;

	Ok(())
}
