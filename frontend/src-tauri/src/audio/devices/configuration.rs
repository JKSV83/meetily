use anyhow::{anyhow, Result};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::atomic::AtomicU64;

lazy_static! {
    pub static ref LAST_AUDIO_CAPTURE: AtomicU64 = AtomicU64::new(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    );
}

#[cfg(target_os = "linux")]
const LINUX_RAW_DEVICE_PREFIXES: [&str; 10] = [
    "default:",
    "sysdefault:",
    "front:",
    "surround",
    "iec958",
    "hw:",
    "plughw:",
    "dmix:",
    "dsnoop:",
    "cards.pcm.",
];

#[derive(Clone, Debug, PartialEq)]
pub enum AudioTranscriptionEngine {
    Deepgram,
    WhisperTiny,
    WhisperDistilLargeV3,
    WhisperLargeV3Turbo,
    WhisperLargeV3,
}

impl fmt::Display for AudioTranscriptionEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AudioTranscriptionEngine::Deepgram => write!(f, "Deepgram"),
            AudioTranscriptionEngine::WhisperTiny => write!(f, "WhisperTiny"),
            AudioTranscriptionEngine::WhisperDistilLargeV3 => write!(f, "WhisperLarge"),
            AudioTranscriptionEngine::WhisperLargeV3Turbo => write!(f, "WhisperLargeV3Turbo"),
            AudioTranscriptionEngine::WhisperLargeV3 => write!(f, "WhisperLargeV3"),
        }
    }
}

impl Default for AudioTranscriptionEngine {
    fn default() -> Self {
        AudioTranscriptionEngine::WhisperLargeV3Turbo
    }
}

#[derive(Clone, Debug)]
pub struct DeviceControl {
    pub is_running: bool,
    pub is_paused: bool,
}

#[derive(Clone, Eq, PartialEq, Hash, Serialize, Debug, Deserialize)]
pub enum DeviceType {
    Input,
    Output,
}

#[derive(Clone, Eq, PartialEq, Hash, Serialize, Debug)]
pub struct AudioDevice {
    pub name: String,
    pub device_type: DeviceType,
}

impl AudioDevice {
    pub fn new(name: String, device_type: DeviceType) -> Self {
        AudioDevice { name, device_type }
    }

    pub fn from_name(name: &str) -> Result<Self> {
        if name.trim().is_empty() {
            return Err(anyhow!("Device name cannot be empty"));
        }

        let (name, device_type) = if name.to_lowercase().ends_with("(input)") {
            (
                name.trim_end_matches("(input)").trim().to_string(),
                DeviceType::Input,
            )
        } else if name.to_lowercase().ends_with("(output)") {
            (
                name.trim_end_matches("(output)").trim().to_string(),
                DeviceType::Output,
            )
        } else {
            return Err(anyhow!(
                "Device type (input/output) not specified in the name"
            ));
        };

        Ok(AudioDevice::new(name, device_type))
    }
}

impl fmt::Display for AudioDevice {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} ({})",
            self.name,
            match self.device_type {
                DeviceType::Input => "input",
                DeviceType::Output => "output",
            }
        )
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn linux_device_is_raw_alias(name: &str) -> bool {
    let lower = name.trim().to_lowercase();
    if lower.is_empty() {
        return false;
    }

    LINUX_RAW_DEVICE_PREFIXES
        .iter()
        .any(|prefix| lower.starts_with(prefix) || lower.contains(prefix))
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn linux_device_is_raw_alias(_name: &str) -> bool {
    false
}

#[cfg(target_os = "linux")]
pub(crate) fn linux_device_is_usable(name: &str) -> bool {
    let lower = name.trim().to_lowercase();
    if lower.is_empty() {
        return false;
    }

    if matches!(lower.as_str(), "default") {
        return true;
    }

    if lower.starts_with("pipewire")
        || lower.contains("pulse")
        || lower.contains("jack")
        || lower.contains("monitor")
        || lower.contains("loopback")
    {
        return true;
    }

    if linux_device_is_raw_alias(&lower) {
        return false;
    }

    true
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn linux_device_is_usable(_name: &str) -> bool {
    true
}

#[cfg(target_os = "linux")]
pub(crate) fn linux_system_audio_source_name(name: &str) -> bool {
    let lower = name.trim().to_lowercase();
    if lower.is_empty() {
        return false;
    }

    if linux_device_is_raw_alias(&lower) {
        return false;
    }

    matches!(lower.as_str(), "default")
        || lower.starts_with("pipewire")
        || lower.contains("pulse")
        || lower.contains("jack")
        || lower.contains("monitor")
        || lower.contains("loopback")
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn linux_system_audio_source_name(_name: &str) -> bool {
    true
}

/// Parse audio device from string name
pub fn parse_audio_device(name: &str) -> Result<AudioDevice> {
    AudioDevice::from_name(name)
}

/// Get device and config for audio operations
pub async fn get_device_and_config(
    audio_device: &AudioDevice,
) -> Result<(cpal::Device, cpal::SupportedStreamConfig)> {
    #[cfg(target_os = "windows")]
    {
        return super::platform::get_windows_device(audio_device);
    }

    #[cfg(not(target_os = "windows"))]
    {
        use cpal::traits::{DeviceTrait, HostTrait};

        let host = cpal::default_host();

        match audio_device.device_type {
            DeviceType::Input => {
                for device in host.input_devices()? {
                    if let Ok(name) = device.name() {
                        if name == audio_device.name {
                            let default_config = device.default_input_config().map_err(|e| {
                                anyhow!("Failed to get default input config: {}", e)
                            })?;
                            return Ok((device, default_config));
                        }
                    }
                }
            }
            DeviceType::Output => {
                #[cfg(target_os = "macos")]
                {
                    // Use default host for all macOS output devices
                    // Core Audio backend uses direct cidre API for system capture, not cpal
                    for device in host.output_devices()? {
                        if let Ok(name) = device.name() {
                            if name == audio_device.name {
                                let default_config = device
                                    .default_output_config()
                                    .map_err(|e| anyhow!("Failed to get output config: {}", e))?;
                                return Ok((device, default_config));
                            }
                        }
                    }
                }

                #[cfg(target_os = "linux")]
                {
                    // Linux system audio sources are exposed as input-capable monitor/loopback devices.
                    for device in host.input_devices()? {
                        if let Ok(name) = device.name() {
                            if name == audio_device.name {
                                let default_config =
                                    device.default_input_config().map_err(|e| {
                                        anyhow!("Failed to get default input config: {}", e)
                                    })?;
                                return Ok((device, default_config));
                            }
                        }
                    }
                }
            }
        }

        Err(anyhow!("Device not found: {}", audio_device.name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "linux")]
    fn linux_filters_raw_alsa_aliases() {
        for name in [
            "default:CARD=Audio",
            "sysdefault:CARD=Audio",
            "front:CARD=Audio",
            "surround51:CARD=Audio",
            "iec958:CARD=Audio",
            "hw:0,0",
            "plughw:0,0",
            "dmix:front",
            "dsnoop:front",
            "cards.pcm.hdmi",
        ] {
            assert!(
                linux_device_is_raw_alias(name),
                "expected raw alias to be rejected: {name}"
            );
            assert!(
                !linux_device_is_usable(name),
                "expected raw alias to be hidden: {name}"
            );
            assert!(
                !linux_system_audio_source_name(name),
                "expected raw alias to stay hidden from system audio: {name}"
            );
        }
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn linux_keeps_useful_logical_endpoints() {
        for name in [
            "default",
            "pipewire",
            "pulse",
            "jack",
            "alsa_output.pci-0000_00_1f.3.analog-stereo.monitor",
        ] {
            assert!(
                linux_device_is_usable(name),
                "expected logical endpoint to be kept: {name}"
            );
            assert!(
                linux_system_audio_source_name(name),
                "expected logical endpoint to be usable for system audio: {name}"
            );
        }

        for name in [
            "Built-in Audio Analog Stereo",
            "USB Audio Device",
            "Headset Mic",
        ] {
            assert!(
                linux_device_is_usable(name),
                "expected human-readable device to be kept: {name}"
            );
        }
    }
}
