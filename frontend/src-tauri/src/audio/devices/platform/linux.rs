use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait};
use log::{debug, warn};
use std::collections::HashSet;
use std::process::Command;

use crate::audio::devices::configuration::{AudioDevice, DeviceType};

#[derive(Clone, Debug)]
struct LinuxMonitorSource {
    name: String,
}

/// Configure Linux audio devices using CPAL for microphones and PulseAudio/PipeWire monitor
/// sources for system audio capture.
pub fn configure_linux_audio(host: &cpal::Host) -> Result<Vec<AudioDevice>> {
    let mut devices = Vec::new();
    let mut seen = HashSet::new();

    for device in host.input_devices()? {
        if let Ok(name) = device.name() {
            if seen.insert((name.clone(), DeviceType::Input)) {
                devices.push(AudioDevice::new(name, DeviceType::Input));
            }
        }
    }

    for source in list_monitor_sources()? {
        if seen.insert((source.name.clone(), DeviceType::Output)) {
            devices.push(AudioDevice::new(source.name, DeviceType::Output));
        }
    }

    Ok(devices)
}

pub fn list_linux_system_audio_devices() -> Result<Vec<AudioDevice>> {
    Ok(list_monitor_sources()?
        .into_iter()
        .map(|source| AudioDevice::new(source.name, DeviceType::Output))
        .collect())
}

pub fn default_linux_system_audio_device() -> Result<AudioDevice> {
    let default_monitor = default_monitor_source_name()?;
    Ok(AudioDevice::new(default_monitor, DeviceType::Output))
}

pub fn resolve_linux_system_audio_device(name: &str) -> Result<AudioDevice> {
    let monitor_sources = list_monitor_sources()?;

    if let Some(source) = monitor_sources.iter().find(|source| source.name == name) {
        return Ok(AudioDevice::new(source.name.clone(), DeviceType::Output));
    }

    let candidate_with_suffix = if name.ends_with(".monitor") {
        name.to_string()
    } else {
        format!("{}.monitor", name)
    };

    if let Some(source) = monitor_sources
        .iter()
        .find(|source| source.name == candidate_with_suffix)
    {
        return Ok(AudioDevice::new(source.name.clone(), DeviceType::Output));
    }

    if let Ok(default_monitor) = default_monitor_source_name() {
        debug!(
            "Linux system audio preference '{}' not found, falling back to default monitor '{}'",
            name, default_monitor
        );
        return Ok(AudioDevice::new(default_monitor, DeviceType::Output));
    }

    Err(anyhow!(
        "No usable Linux system-audio monitor source found for '{}'",
        name
    ))
}

pub fn linux_system_audio_available() -> bool {
    list_monitor_sources()
        .map(|sources| !sources.is_empty())
        .unwrap_or(false)
}

fn default_monitor_source_name() -> Result<String> {
    let sources = list_monitor_sources()?;
    if sources.is_empty() {
        return Err(anyhow!(
            "No PulseAudio/PipeWire monitor sources available for system audio capture"
        ));
    }

    if let Ok(default_sink) = default_sink_name() {
        let expected_monitor = format!("{}.monitor", default_sink);
        if sources.iter().any(|source| source.name == expected_monitor) {
            return Ok(expected_monitor);
        }

        warn!(
            "Default sink '{}' did not expose monitor '{}', falling back to first available monitor",
            default_sink, expected_monitor
        );
    }

    Ok(sources[0].name.clone())
}

fn default_sink_name() -> Result<String> {
    let output = Command::new("pactl")
        .args(["info"])
        .output()
        .map_err(|e| anyhow!("Failed to run pactl info: {}", e))?;

    if !output.status.success() {
        return Err(anyhow!(
            "pactl info failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(value) = line.strip_prefix("Default Sink: ") {
            let sink_name = value.trim();
            if !sink_name.is_empty() {
                return Ok(sink_name.to_string());
            }
        }
    }

    Err(anyhow!(
        "Unable to determine default PulseAudio/PipeWire sink"
    ))
}

fn list_monitor_sources() -> Result<Vec<LinuxMonitorSource>> {
    let output = Command::new("pactl")
        .args(["list", "short", "sources"])
        .output()
        .map_err(|e| anyhow!("Failed to run pactl list short sources: {}", e))?;

    if !output.status.success() {
        return Err(anyhow!(
            "pactl list short sources failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut sources = Vec::new();

    for line in stdout.lines() {
        let mut parts = line.split_whitespace();
        let _index = parts.next();
        let source_name = match parts.next() {
            Some(name) if name.ends_with(".monitor") => name,
            _ => continue,
        };

        sources.push(LinuxMonitorSource {
            name: source_name.to_string(),
        });
    }

    if sources.is_empty() {
        return Err(anyhow!(
            "No PulseAudio/PipeWire monitor sources found. System audio capture requires a monitor source exposed by the active audio server"
        ));
    }

    Ok(sources)
}
