use std::ffi::c_void;

type AudioObjectId = u32;
type AudioStatus = i32;

#[repr(C)]
#[derive(Clone, Copy)]
struct AudioObjectPropertyAddress {
	selector: u32,
	scope: u32,
	element: u32,
}

#[link(name = "CoreAudio", kind = "framework")]
unsafe extern "C" {
	fn AudioObjectGetPropertyData(
		object_id: AudioObjectId,
		address: *const AudioObjectPropertyAddress,
		qualifier_data_size: u32,
		qualifier_data: *const c_void,
		data_size: *mut u32,
		data: *mut c_void,
	) -> AudioStatus;
	fn AudioObjectIsPropertySettable(object_id: AudioObjectId, address: *const AudioObjectPropertyAddress, is_settable: *mut u8) -> AudioStatus;
	fn AudioObjectSetPropertyData(
		object_id: AudioObjectId,
		address: *const AudioObjectPropertyAddress,
		qualifier_data_size: u32,
		qualifier_data: *const c_void,
		data_size: u32,
		data: *const c_void,
	) -> AudioStatus;
}

const SYSTEM_OBJECT: AudioObjectId = 1;

fn four_cc(value: &[u8; 4]) -> u32 {
	u32::from_be_bytes(*value)
}

fn property(selector: &[u8; 4], scope: &[u8; 4]) -> AudioObjectPropertyAddress {
	AudioObjectPropertyAddress {
		selector: four_cc(selector),
		scope: four_cc(scope),
		element: 0,
	}
}

fn check_status(operation: &str, status: AudioStatus) -> Result<(), anyhow::Error> {
	if status == 0 {
		Ok(())
	} else {
		Err(anyhow::anyhow!("Core Audio {operation} failed with status {status}"))
	}
}

pub fn toggle_default_input_mute() -> Result<(), anyhow::Error> {
	let default_input = property(b"dIn ", b"glob");
	let mut device_id = 0;
	let mut device_size = std::mem::size_of::<AudioObjectId>() as u32;
	check_status("default-input lookup", unsafe {
		AudioObjectGetPropertyData(SYSTEM_OBJECT, &default_input, 0, std::ptr::null(), &mut device_size, (&mut device_id as *mut AudioObjectId).cast())
	})?;
	if device_id == 0 {
		return Err(anyhow::anyhow!("macOS has no active default input device"));
	}

	let mute_address = property(b"mute", b"inpt");
	let mut is_settable = 0u8;
	check_status("microphone-mute capability lookup", unsafe {
		AudioObjectIsPropertySettable(device_id, &mute_address, &mut is_settable)
	})?;
	if is_settable == 0 {
		return Err(anyhow::anyhow!("The current default input device does not expose a writable microphone-mute control"));
	}

	let mut current_mute = 0u32;
	let mut mute_size = std::mem::size_of::<u32>() as u32;
	check_status("microphone-mute read", unsafe {
		AudioObjectGetPropertyData(device_id, &mute_address, 0, std::ptr::null(), &mut mute_size, (&mut current_mute as *mut u32).cast())
	})?;
	let next_mute = u32::from(current_mute == 0);
	check_status("microphone-mute update", unsafe {
		AudioObjectSetPropertyData(device_id, &mute_address, 0, std::ptr::null(), std::mem::size_of::<u32>() as u32, (&next_mute as *const u32).cast())
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn core_audio_addresses_use_default_input_and_input_mute_selectors() {
		let default_input = property(b"dIn ", b"glob");
		assert_eq!(default_input.selector, 0x6449_6e20);
		assert_eq!(default_input.scope, 0x676c_6f62);
		let mute = property(b"mute", b"inpt");
		assert_eq!(mute.selector, 0x6d75_7465);
		assert_eq!(mute.scope, 0x696e_7074);
		assert_eq!(mute.element, 0);
	}
}
