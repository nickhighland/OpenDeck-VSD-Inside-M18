use super::{GenericInstancePayload, send_to_plugin};

use crate::events::frontend::instances::{key_moved, show_alert, update_state};
use crate::shared::{ActionContext, Context};
use crate::store::profiles::{acquire_locks_mut, get_slot_mut, mark_profile_stale};

use std::sync::LazyLock;
use std::time::Duration;
use std::{future::Future, pin::Pin};

use dashmap::DashMap;
use serde::Serialize;
use tokio::time::Instant;

static KEY_DOWN_TARGETS: LazyLock<DashMap<(String, u8), Context>> = LazyLock::new(DashMap::new);

#[derive(Serialize)]
struct KeyEvent {
	event: &'static str,
	action: String,
	context: ActionContext,
	device: String,
	payload: GenericInstancePayload,
}

fn action_down<'a>(instance: &'a mut crate::shared::ActionInstance) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + Send + 'a>> {
	Box::pin(async move {
		match instance.action.uuid.as_str() {
			"opendeck.multiaction" => {
				let delays = instance.settings.get("delays").and_then(serde_json::Value::as_array).cloned().unwrap_or_default();
				let child_count = instance.children.as_ref().map(Vec::len).unwrap_or(0);
				if let Some(children) = &mut instance.children {
					for (index, child) in children.iter_mut().enumerate() {
						run_composite_child(child).await?;
						if index + 1 < child_count {
							let delay = sequence_delay_ms(&delays, index);
							if delay > 0 {
								tokio::time::sleep(Duration::from_millis(delay)).await;
							}
						}
					}
				}
				Ok(())
			}
			"opendeck.toggleaction" | "opendeck.carouselaction" => {
				if let Some(children) = &mut instance.children
					&& !children.is_empty()
				{
					let index = (instance.current_state as usize).min(children.len() - 1);
					action_down(&mut children[index]).await?;
				}
				Ok(())
			}
			_ if crate::m18::is_led_action(&instance.action.uuid) => crate::m18::apply_led_action(instance).await,
			_ if crate::m18_actions::is_native_action(&instance.action.uuid) => crate::m18_actions::key_down(instance).await,
			_ => {
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
				.await
			}
		}
	})
}

fn action_up<'a>(instance: &'a mut crate::shared::ActionInstance) -> Pin<Box<dyn Future<Output = Result<bool, anyhow::Error>> + Send + 'a>> {
	Box::pin(async move {
		match instance.action.uuid.as_str() {
			"opendeck.multiaction" => Ok(false),
			"opendeck.toggleaction" | "opendeck.carouselaction" => {
				let Some(children) = &mut instance.children else { return Ok(false) };
				if children.is_empty() {
					return Ok(false);
				}
				let index = (instance.current_state as usize).min(children.len() - 1);
				let child = &mut children[index];
				if action_up(child).await? && child.states.len() > 1 && !child.action.disable_automatic_states {
					child.current_state = (child.current_state + 1) % child.states.len() as u16;
				}
				instance.current_state = ((index + 1) % children.len()) as u16;
				Ok(false)
			}
			_ if crate::m18::is_led_action(&instance.action.uuid) => Ok(false),
			_ if crate::m18_actions::is_native_action(&instance.action.uuid) => crate::m18_actions::key_up(instance).await,
			_ => {
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
				Ok(instance.states.len() == 2 && !instance.action.disable_automatic_states)
			}
		}
	})
}

fn sync_runtime_states(stored: &mut crate::shared::ActionInstance, completed: &crate::shared::ActionInstance) {
	stored.current_state = completed.current_state;
	if let (Some(stored_children), Some(completed_children)) = (&mut stored.children, &completed.children) {
		for (stored, completed) in stored_children.iter_mut().zip(completed_children) {
			sync_runtime_states(stored, completed);
		}
	}
}

fn collect_contexts(instance: &crate::shared::ActionInstance, contexts: &mut Vec<ActionContext>) {
	contexts.push(instance.context.clone());
	if let Some(children) = &instance.children {
		for child in children {
			collect_contexts(child, contexts);
		}
	}
}

fn milliseconds(value: Option<&serde_json::Value>) -> u64 {
	value.and_then(|value| value.as_u64().or_else(|| value.as_str()?.parse::<u64>().ok())).unwrap_or(0).min(300_000)
}

fn sequence_delay_ms(delays: &[serde_json::Value], index: usize) -> u64 {
	delays.get(index).map(|value| milliseconds(Some(value))).unwrap_or(100)
}

fn child_delay(child: &crate::shared::ActionInstance, field: &str) -> u64 {
	milliseconds(child.settings.get("_vsdMultiActionDelays").and_then(|delays| delays.get(field)))
}

/// Runs one step of a Multi Action. A failing step is reported on its key and
/// logged, but never aborts the remaining steps of the sequence.
async fn run_composite_child(child: &mut crate::shared::ActionInstance) -> Result<(), anyhow::Error> {
	tokio::time::sleep(Duration::from_millis(child_delay(child, "Delay1"))).await;
	if child.action.uuid.eq_ignore_ascii_case("com.hotspot.streamdock.multiactions.delay") {
		let delay = milliseconds(
			child
				.settings
				.get("delay")
				.or_else(|| child.settings.get("Delay"))
				.or_else(|| child.settings.get("time"))
				.or_else(|| child.settings.get("Time")),
		);
		tokio::time::sleep(Duration::from_millis(delay)).await;
	} else {
		if let Err(error) = action_down(child).await {
			report_action_failure(child, "press", &error);
		}
		tokio::time::sleep(Duration::from_millis(100)).await;
		match action_up(child).await {
			Ok(true) if child.states.len() > 1 && !child.action.disable_automatic_states => {
				child.current_state = (child.current_state + 1) % child.states.len() as u16;
			}
			Ok(_) => {}
			Err(error) => report_action_failure(child, "release", &error),
		}
	}
	tokio::time::sleep(Duration::from_millis(child_delay(child, "Delay2"))).await;
	Ok(())
}

/// Log a failed action and flash the alert badge on its key, both in the
/// editor and on the M18 LCD, so failures are visible instead of silent.
fn report_action_failure(instance: &crate::shared::ActionInstance, phase: &str, error: &anyhow::Error) {
	log::warn!("{} ({}) {phase} failed: {error:#}", instance.action.name, instance.action.uuid);
	show_alert(&instance.context);
}

pub async fn key_down_at(device: &str, key: u8, requested_at: Instant) -> Result<(), anyhow::Error> {
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
		let led_instance = instance.clone();
		drop(locks);
		if let Err(error) = crate::m18::apply_led_action(&led_instance).await {
			report_action_failure(&led_instance, "press", &error);
		}
		return Ok(());
	}
	if crate::m18_actions::is_native_action(&instance.action.uuid) {
		let native_instance = instance.clone();
		drop(locks);
		if let Err(error) = crate::m18_actions::key_down_at(&native_instance, requested_at).await {
			report_action_failure(&native_instance, "press", &error);
		}
		return Ok(());
	}
	if instance.action.uuid == "opendeck.multiaction" {
		let mut completed = instance.clone();
		drop(locks);
		action_down(&mut completed).await?;

		let mut locks = acquire_locks_mut().await;
		let contexts = if let Some(stored) = get_slot_mut(&context, &mut locks).await? {
			sync_runtime_states(stored, &completed);
			let mut contexts = Vec::new();
			collect_contexts(stored, &mut contexts);
			contexts
		} else {
			vec![]
		};
		for child_context in contexts {
			let _ = update_state(crate::APP_HANDLE.get().unwrap(), child_context, &mut locks).await;
		}

		mark_profile_stale(device, &mut locks).await?;
	} else if matches!(instance.action.uuid.as_str(), "opendeck.toggleaction" | "opendeck.carouselaction") {
		let Some(children) = instance.children.as_ref().filter(|children| !children.is_empty()) else {
			return Ok(());
		};
		let mut child = children[(instance.current_state as usize).min(children.len() - 1)].clone();
		let child_index = child.context.index;
		drop(locks);
		if let Err(error) = action_down(&mut child).await {
			report_action_failure(&child, "press", &error);
		}
		let mut locks = acquire_locks_mut().await;
		let contexts = if let Some(parent) = get_slot_mut(&context, &mut locks).await?
			&& let Some(children) = &mut parent.children
			&& let Some(stored_child) = children.iter_mut().find(|stored| stored.context.index == child_index)
		{
			sync_runtime_states(stored_child, &child);
			let mut contexts = Vec::new();
			collect_contexts(stored_child, &mut contexts);
			contexts
		} else {
			vec![]
		};
		for child_context in contexts {
			let _ = update_state(crate::APP_HANDLE.get().unwrap(), child_context, &mut locks).await;
		}
		mark_profile_stale(device, &mut locks).await?;
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
		// A failing native action (a missing app, a denied Apple Event, a
		// cancelled screenshot) is reported on its key; it must never propagate
		// into the device session.
		let advances_state = match crate::m18_actions::key_up(&native_instance).await {
			Ok(advances_state) => advances_state,
			Err(error) => {
				report_action_failure(&native_instance, "release", &error);
				false
			}
		};
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
				// The core draws first so that the editor's redraw, which adds
				// the title, is the one left on the M18.
				let _ = crate::m18_actions::render(&instance).await;
				let _ = update_state(crate::APP_HANDLE.get().unwrap(), instance.context.clone(), &mut locks).await;
			}
			mark_profile_stale(device, &mut locks).await?;
		}
		return Ok(());
	}

	if matches!(instance.action.uuid.as_str(), "opendeck.toggleaction" | "opendeck.carouselaction") {
		let Some(children) = instance.children.as_ref().filter(|children| !children.is_empty()) else {
			return Ok(());
		};
		let index = (instance.current_state as usize).min(children.len() - 1);
		let mut child = children[index].clone();
		drop(locks);
		let advances_child_state = match action_up(&mut child).await {
			Ok(advances) => advances,
			Err(error) => {
				report_action_failure(&child, "release", &error);
				false
			}
		};
		if advances_child_state && child.states.len() > 1 && !child.action.disable_automatic_states {
			child.current_state = (child.current_state + 1) % child.states.len() as u16;
		}
		let mut locks = acquire_locks_mut().await;
		let (child_context, updated_parent_context) = if let Some(parent) = get_slot_mut(&context, &mut locks).await?
			&& let Some(children) = &mut parent.children
		{
			let child_context = if let Some(stored_child) = children.iter_mut().find(|stored| stored.context.index == child.context.index) {
				sync_runtime_states(stored_child, &child);
				Some(stored_child.context.clone())
			} else {
				None
			};
			parent.current_state = ((index + 1) % children.len().max(1)) as u16;
			(child_context, Some(parent.context.clone()))
		} else {
			(None, None)
		};
		if let Some(child_context) = child_context {
			let _ = update_state(crate::APP_HANDLE.get().unwrap(), child_context, &mut locks).await;
		}
		if let Some(parent_context) = updated_parent_context {
			let _ = update_state(crate::APP_HANDLE.get().unwrap(), parent_context, &mut locks).await;
		}
		mark_profile_stale(device, &mut locks).await?;
		return Ok(());
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

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn open_deck_multi_action_keeps_the_default_inter_child_delay() {
		let delays = vec![serde_json::json!(0), serde_json::json!("240")];
		assert_eq!(sequence_delay_ms(&[], 0), 100);
		assert_eq!(sequence_delay_ms(&delays, 0), 0);
		assert_eq!(sequence_delay_ms(&delays, 1), 240);
		assert_eq!(sequence_delay_ms(&delays, 2), 100);
	}
}
