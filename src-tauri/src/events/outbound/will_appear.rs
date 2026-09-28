use super::{GenericInstancePayload, send_to_plugin};

use crate::shared::{ActionContext, ActionInstance};

#[derive(serde::Serialize)]
struct AppearEvent {
	event: &'static str,
	action: String,
	context: ActionContext,
	device: String,
	payload: GenericInstancePayload,
}

fn is_composite(instance: &ActionInstance) -> bool {
	matches!(instance.action.uuid.as_str(), "opendeck.multiaction" | "opendeck.toggleaction" | "opendeck.carouselaction")
}

pub async fn will_appear(instance: &ActionInstance) -> Result<(), anyhow::Error> {
	if is_composite(instance) {
		return Ok(());
	}
	if crate::m18::is_led_action(&instance.action.uuid) {
		crate::m18::apply_led_action(instance).await?;
		return Ok(());
	}
	if crate::m18_actions::is_native_action(&instance.action.uuid) {
		crate::m18_actions::render(instance).await?;
		return Ok(());
	}
	send_to_plugin(
		&instance.action.plugin,
		&AppearEvent {
			event: "willAppear",
			action: instance.action.uuid.clone(),
			context: instance.context.clone(),
			device: instance.context.device.clone(),
			payload: GenericInstancePayload::new(instance),
		},
	)
	.await?;

	super::states::title_parameters_did_change(instance, instance.current_state).await?;

	Ok(())
}

pub async fn will_disappear(instance: &ActionInstance, clear_on_device: bool) -> Result<(), anyhow::Error> {
	if is_composite(instance) {
		return Ok(());
	}
	if crate::m18::is_led_action(&instance.action.uuid) {
		if clear_on_device {
			crate::events::outbound::devices::update_image((&instance.context).into(), None).await?;
		}
		return Ok(());
	}
	if crate::m18_actions::is_native_action(&instance.action.uuid) {
		if clear_on_device {
			let _ = crate::events::outbound::devices::update_image((&instance.context).into(), None).await;
		}
		return Ok(());
	}
	send_to_plugin(
		&instance.action.plugin,
		&AppearEvent {
			event: "willDisappear",
			action: instance.action.uuid.clone(),
			context: instance.context.clone(),
			device: instance.context.device.clone(),
			payload: GenericInstancePayload::new(instance),
		},
	)
	.await?;

	if clear_on_device && let Err(error) = crate::events::outbound::devices::update_image((&instance.context).into(), None).await {
		log::warn!("Failed to clear device image: {}", error);
	}

	Ok(())
}
