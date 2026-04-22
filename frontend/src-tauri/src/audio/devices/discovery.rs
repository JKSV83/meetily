use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use log::error;
use std::collections::HashSet;

use super::configuration::{linux_device_is_usable, AudioDevice, DeviceType};
use super::platform;

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

/// List all available audio devices on the system
pub async fn list_audio_devices() -> Result<Vec<AudioDevice>> {
    let host = cpal::default_host();

    // Platform-specific device enumeration
    let mut devices = {
        #[cfg(target_os = "windows")]
        {
            platform::configure_windows_audio(&host)?
        }

        #[cfg(target_os = "linux")]
        {
            platform::configure_linux_audio(&host)?
        }

        #[cfg(target_os = "macos")]
        {
            platform::configure_macos_audio(&host)?
        }
    };

    #[cfg(target_os = "linux")]
    {
        // Linux already got the curated pass, keep fallback narrow so ALSA aliases
        // do not get reintroduced into the picker.
        if devices.is_empty() {
            let mut seen: HashSet<(String, DeviceType)> = devices
                .iter()
                .map(|device| (device.name.clone(), device.device_type.clone()))
                .collect();

            if let Ok(input_devices) = host.input_devices() {
                for device in input_devices {
                    if let Ok(name) = device.name() {
                        if linux_device_is_usable(&name) {
                            push_unique_device(
                                &mut devices,
                                &mut seen,
                                name.clone(),
                                DeviceType::Input,
                            );
                        }

                        if super::configuration::linux_system_audio_source_name(&name) {
                            push_unique_device(&mut devices, &mut seen, name, DeviceType::Output);
                        }
                    }
                }
            }
        }

        Ok(devices)
    }

    #[cfg(not(target_os = "linux"))]
    {
        // Add any additional devices from the default host
        if let Ok(other_devices) = host.devices() {
            let mut seen: HashSet<(String, DeviceType)> = devices
                .iter()
                .map(|device| (device.name.clone(), device.device_type.clone()))
                .collect();

            for device in other_devices {
                if let Ok(name) = device.name() {
                    if !seen.contains(&(name.clone(), DeviceType::Output)) {
                        push_unique_device(&mut devices, &mut seen, name, DeviceType::Output);
                    }
                }
            }
        }

        Ok(devices)
    }
}

/// Trigger audio permission request on platforms that require it
/// Returns Ok(true) if permission is granted, Ok(false) if denied, Err if something went wrong
pub fn trigger_audio_permission() -> Result<bool> {
    use log::info;

    let host = cpal::default_host();
    let device = match host.default_input_device() {
        Some(d) => d,
        None => {
            info!("[trigger_audio_permission] No default input device found - permission likely denied");
            return Ok(false);
        }
    };

    let config = match device.default_input_config() {
        Ok(c) => c,
        Err(e) => {
            info!("[trigger_audio_permission] Failed to get input config: {} - permission likely denied", e);
            return Ok(false);
        }
    };

    // Build and start an input stream to trigger the permission request
    let stream = match device.build_input_stream(
        &config.into(),
        |_data: &[f32], _: &cpal::InputCallbackInfo| {
            // Do nothing, we just want to trigger the permission request
        },
        |err| error!("Error in audio stream: {}", err),
        None,
    ) {
        Ok(s) => s,
        Err(e) => {
            info!("[trigger_audio_permission] Failed to build input stream: {} - permission likely denied", e);
            return Ok(false);
        }
    };

    // Start the stream to actually trigger the permission dialog
    if let Err(e) = stream.play() {
        info!(
            "[trigger_audio_permission] Failed to play stream: {} - permission likely denied",
            e
        );
        return Ok(false);
    }

    // Sleep briefly to allow the permission dialog to appear and for stream to actually work
    std::thread::sleep(std::time::Duration::from_millis(500));

    // If we got here, permission was granted
    info!("[trigger_audio_permission] Stream played successfully - permission granted");

    // Stop the stream
    drop(stream);

    Ok(true)
}
