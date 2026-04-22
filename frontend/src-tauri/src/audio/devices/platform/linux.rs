use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait};
use std::collections::HashSet;

use crate::audio::devices::configuration::{
    linux_device_is_usable, linux_system_audio_source_name, AudioDevice, DeviceType,
};

fn push_unique_device(
    devices: &mut Vec<AudioDevice>,
    seen: &mut HashSet<(String, DeviceType)>,
    name: String,
    device_type: DeviceType,
) {
    let key = (name.clone(), device_type.clone());
    if seen.insert(key) {
        devices.push(AudioDevice::new(name, device_type));
    }
}

/// Configure Linux audio devices using ALSA/PulseAudio
pub fn configure_linux_audio(host: &cpal::Host) -> Result<Vec<AudioDevice>> {
    let mut devices = Vec::new();
    let mut seen: HashSet<(String, DeviceType)> = HashSet::new();

    // Add usable input devices
    for device in host.input_devices()? {
        if let Ok(name) = device.name() {
            if linux_device_is_usable(&name) {
                push_unique_device(&mut devices, &mut seen, name, DeviceType::Input);
            }
        }
    }

    // Add only capture-safe system audio sources, not raw ALSA playback aliases
    for device in host.input_devices()? {
        if let Ok(name) = device.name() {
            if linux_system_audio_source_name(&name) {
                push_unique_device(&mut devices, &mut seen, name, DeviceType::Output);
            }
        }
    }

    Ok(devices)
}
