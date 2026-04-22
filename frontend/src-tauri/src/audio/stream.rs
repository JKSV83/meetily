use anyhow::Result;
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{BufferSize, Device, Stream, StreamConfig, SupportedBufferSize, SupportedStreamConfig};
use log::{error, info, warn};
use std::sync::Arc;
use tokio::sync::mpsc;

use super::capture::{get_current_backend, AudioCaptureBackend};
use super::devices::{get_device_and_config, AudioDevice};
use super::pipeline::AudioCapture;
use super::recording_state::{DeviceType, RecordingState};

#[cfg(target_os = "macos")]
use super::capture::CoreAudioCapture;
#[cfg(target_os = "linux")]
use super::capture::SystemAudioCapture;

/// Stream backend implementation
pub enum StreamBackend {
    /// CPAL-based stream (ScreenCaptureKit or default)
    Cpal(Stream),
    /// Core Audio direct implementation (macOS only)
    #[cfg(target_os = "macos")]
    CoreAudio {
        task: Option<tokio::task::JoinHandle<()>>,
    },
    /// PulseAudio/PipeWire monitor-source capture (Linux only)
    #[cfg(target_os = "linux")]
    LinuxSystemAudio {
        task: Option<tokio::task::JoinHandle<()>>,
    },
}

// SAFETY: While Stream doesn't implement Send, we ensure it's only accessed
// from the same thread context by using spawn_blocking for operations that cross thread boundaries
unsafe impl Send for StreamBackend {}

/// Simplified audio stream wrapper with multi-backend support
pub struct AudioStream {
    device: Arc<AudioDevice>,
    backend: StreamBackend,
}

// SAFETY: AudioStream contains StreamBackend which we've marked as Send
unsafe impl Send for AudioStream {}

impl AudioStream {
    /// Create a new audio stream for the given device
    pub async fn create(
        device: Arc<AudioDevice>,
        state: Arc<RecordingState>,
        device_type: DeviceType,
        recording_sender: Option<mpsc::UnboundedSender<super::recording_state::AudioChunk>>,
    ) -> Result<Self> {
        // Get current backend from global config
        let backend_type = get_current_backend();
        Self::create_with_backend(device, state, device_type, recording_sender, backend_type).await
    }

    /// Create a new audio stream with explicit backend selection
    pub async fn create_with_backend(
        device: Arc<AudioDevice>,
        state: Arc<RecordingState>,
        device_type: DeviceType,
        recording_sender: Option<mpsc::UnboundedSender<super::recording_state::AudioChunk>>,
        backend_type: AudioCaptureBackend,
    ) -> Result<Self> {
        info!(
            "🎵 Stream: Creating audio stream for device: {} with backend: {:?}, device_type: {:?}",
            device.name, backend_type, device_type
        );

        // For system audio devices, use the selected backend
        // For microphone devices, always use CPAL
        #[cfg(target_os = "macos")]
        let use_core_audio =
            device_type == DeviceType::System && backend_type == AudioCaptureBackend::CoreAudio;

        #[cfg(not(target_os = "macos"))]
        let use_core_audio = false;

        #[cfg(target_os = "macos")]
        info!(
            "🎵 Stream: use_core_audio = {}, device_type == System: {}, backend == CoreAudio: {}",
            use_core_audio,
            device_type == DeviceType::System,
            backend_type == AudioCaptureBackend::CoreAudio
        );

        #[cfg(not(target_os = "macos"))]
        info!(
            "🎵 Stream: use_core_audio = {}, device_type == System: {}",
            use_core_audio,
            device_type == DeviceType::System
        );

        #[cfg(target_os = "macos")]
        if use_core_audio {
            info!("🎵 Stream: Using Core Audio backend (cidre) for system audio");
            return Self::create_core_audio_stream(device, state, device_type, recording_sender)
                .await;
        }

        #[cfg(target_os = "linux")]
        if device_type == DeviceType::System {
            info!("🎵 Stream: Using Linux PulseAudio/PipeWire monitor-source backend for system audio");
            return Self::create_linux_system_audio_stream(
                device,
                state,
                device_type,
                recording_sender,
            )
            .await;
        }

        // Default path: use CPAL
        #[cfg(target_os = "macos")]
        let backend_name = if backend_type == AudioCaptureBackend::ScreenCaptureKit {
            "ScreenCaptureKit"
        } else {
            "CPAL (default)"
        };

        #[cfg(not(target_os = "macos"))]
        let backend_name = "CPAL";

        info!(
            "🎵 Stream: Using CPAL backend ({}) for device: {}",
            backend_name, device.name
        );
        Self::create_cpal_stream(device, state, device_type, recording_sender).await
    }

    /// Create a CPAL-based stream (ScreenCaptureKit on macOS)
    async fn create_cpal_stream(
        device: Arc<AudioDevice>,
        state: Arc<RecordingState>,
        device_type: DeviceType,
        recording_sender: Option<mpsc::UnboundedSender<super::recording_state::AudioChunk>>,
    ) -> Result<Self> {
        info!("Creating CPAL stream for device: {}", device.name);

        // Get the underlying cpal device and config
        let (cpal_device, config) = get_device_and_config(&device).await?;

        info!(
            "Audio config - Sample rate: {}, Channels: {}, Format: {:?}",
            config.sample_rate().0,
            config.channels(),
            config.sample_format()
        );

        // Create audio capture processor
        let capture = AudioCapture::new(
            device.clone(),
            state.clone(),
            config.sample_rate().0,
            config.channels(),
            device_type,
            recording_sender,
        );

        // Build the appropriate stream based on sample format
        let stream = Self::build_stream(device.as_ref(), &cpal_device, &config, capture.clone())?;

        // Start the stream
        stream.play()?;
        info!("CPAL stream started for device: {}", device.name);

        Ok(Self {
            device,
            backend: StreamBackend::Cpal(stream),
        })
    }

    /// Create a Core Audio stream (macOS only)
    #[cfg(target_os = "macos")]
    async fn create_core_audio_stream(
        device: Arc<AudioDevice>,
        state: Arc<RecordingState>,
        device_type: DeviceType,
        recording_sender: Option<mpsc::UnboundedSender<super::recording_state::AudioChunk>>,
    ) -> Result<Self> {
        info!(
            "🔊 Stream: Creating Core Audio stream for device: {}",
            device.name
        );

        // Create Core Audio capture
        info!("🔊 Stream: Calling CoreAudioCapture::new()...");
        let capture_impl = CoreAudioCapture::new().map_err(|e| {
            error!("❌ Stream: CoreAudioCapture::new() failed: {}", e);
            anyhow::anyhow!("Failed to create Core Audio capture: {}", e)
        })?;

        info!("✅ Stream: CoreAudioCapture created, calling stream()...");
        let core_stream = capture_impl.stream().map_err(|e| {
            error!("❌ Stream: capture_impl.stream() failed: {}", e);
            anyhow::anyhow!("Failed to create Core Audio stream: {}", e)
        })?;

        let sample_rate = core_stream.sample_rate();
        info!(
            "✅ Stream: Core Audio stream created with sample rate: {} Hz",
            sample_rate
        );

        // Create audio capture processor for pipeline integration
        // CRITICAL: Core Audio tap is MONO (with_mono_global_tap_excluding_processes)
        let capture = AudioCapture::new(
            device.clone(),
            state.clone(),
            sample_rate,
            1, // Core Audio tap is MONO (not stereo!)
            device_type,
            recording_sender,
        );

        // Spawn task to process Core Audio stream samples
        // The stream needs to be polled continuously to produce samples
        let device_name = device.name.clone();
        info!("🔊 Stream: Spawning tokio task to poll Core Audio stream...");
        let task = tokio::spawn({
            let capture = capture.clone();
            let mut stream = core_stream;

            async move {
                use futures_util::StreamExt;

                let mut buffer = Vec::new();
                let mut frame_count = 0;
                let frames_per_chunk = 1024; // Process in chunks of 1024 samples

                info!(
                    "✅ Stream: Core Audio processing task started for {}",
                    device_name
                );

                let mut _sample_count = 0u64;
                while let Some(sample) = stream.next().await {
                    _sample_count += 1;
                    // if _sample_count % 48000 == 0 {
                    //     info!("📊 Stream: Received {} samples from Core Audio stream", _sample_count);
                    // }

                    buffer.push(sample);
                    frame_count += 1;

                    // Process when we have enough samples
                    if frame_count >= frames_per_chunk {
                        capture.process_audio_data(&buffer);
                        buffer.clear();
                        frame_count = 0;
                    }
                }

                // Process any remaining samples
                if !buffer.is_empty() {
                    capture.process_audio_data(&buffer);
                }

                info!(
                    "⚠️ Stream: Core Audio processing task ended for {}",
                    device_name
                );
            }
        });

        info!(
            "✅ Stream: Core Audio stream fully initialized for device: {}",
            device.name
        );

        Ok(Self {
            device: device.clone(),
            backend: StreamBackend::CoreAudio { task: Some(task) },
        })
    }

    #[cfg(target_os = "linux")]
    async fn create_linux_system_audio_stream(
        device: Arc<AudioDevice>,
        state: Arc<RecordingState>,
        device_type: DeviceType,
        recording_sender: Option<mpsc::UnboundedSender<super::recording_state::AudioChunk>>,
    ) -> Result<Self> {
        info!(
            "Creating Linux system audio stream for monitor source: {}",
            device.name
        );

        let capture_impl = SystemAudioCapture::new()?;
        let mut system_stream =
            capture_impl.start_system_audio_capture_for_device(Some(&device.name))?;
        let sample_rate = system_stream.sample_rate();

        let capture = AudioCapture::new(
            device.clone(),
            state.clone(),
            sample_rate,
            2,
            device_type,
            recording_sender,
        );

        let device_name = device.name.clone();
        let task = tokio::spawn(async move {
            use futures_util::StreamExt;

            let mut buffer = Vec::new();
            let frames_per_chunk = 1024;
            let chunk_size = frames_per_chunk * 2;

            info!(
                "✅ Linux system audio monitor task started for {}",
                device_name
            );

            while let Some(sample) = system_stream.next().await {
                buffer.push(sample);
                if buffer.len() >= chunk_size {
                    capture.process_audio_data(&buffer);
                    buffer.clear();
                }
            }

            if !buffer.is_empty() {
                capture.process_audio_data(&buffer);
            }

            info!(
                "⚠️ Linux system audio monitor task ended for {}",
                device_name
            );
        });

        Ok(Self {
            device,
            backend: StreamBackend::LinuxSystemAudio { task: Some(task) },
        })
    }

    #[cfg(target_os = "linux")]
    fn linux_buffer_override_frames() -> Option<u32> {
        match std::env::var("MEETILY_LINUX_BUFFER_FRAMES") {
            Ok(value) => match value.trim().parse::<u32>() {
                Ok(parsed) if parsed > 0 => Some(parsed),
                Ok(_) => {
                    warn!(
                        "Ignoring MEETILY_LINUX_BUFFER_FRAMES='{}' because it must be greater than 0",
                        value
                    );
                    None
                }
                Err(err) => {
                    warn!(
                        "Ignoring MEETILY_LINUX_BUFFER_FRAMES='{}' because it is not a valid frame count: {}",
                        value, err
                    );
                    None
                }
            },
            Err(_) => None,
        }
    }

    #[cfg(target_os = "linux")]
    fn clamp_linux_buffer_frames(
        requested_frames: u32,
        supported_buffer: &SupportedBufferSize,
    ) -> u32 {
        match supported_buffer {
            SupportedBufferSize::Range { min, max } => requested_frames.clamp(*min, *max),
            SupportedBufferSize::Unknown => requested_frames.max(1),
        }
    }

    #[cfg(target_os = "linux")]
    fn linux_stream_config(device: &AudioDevice, config: &SupportedStreamConfig) -> StreamConfig {
        let supported_buffer = *config.buffer_size();
        let mut stream_config: StreamConfig = config.clone().into();

        if let Some(requested_frames) = Self::linux_buffer_override_frames() {
            let clamped_frames =
                Self::clamp_linux_buffer_frames(requested_frames, &supported_buffer);
            stream_config.buffer_size = BufferSize::Fixed(clamped_frames);

            info!(
                "🎛️ Linux CPAL stream for '{}' using MEETILY_LINUX_BUFFER_FRAMES={} (clamped to {}), supported buffer range: {:?}",
                device.name, requested_frames, clamped_frames, supported_buffer
            );
        } else {
            // Keep BufferSize::Default explicitly on Linux so we do not accidentally
            // start forcing tiny ALSA/PipeWire buffer sizes during future refactors.
            stream_config.buffer_size = BufferSize::Default;

            info!(
                "🎛️ Linux CPAL stream for '{}' using BufferSize::Default (supported buffer range: {:?}) to avoid forcing a smaller capture quantum",
                device.name, supported_buffer
            );
        }

        stream_config
    }

    /// Build stream based on sample format
    fn build_stream(
        audio_device: &AudioDevice,
        device: &Device,
        config: &SupportedStreamConfig,
        capture: AudioCapture,
    ) -> Result<Stream> {
        let stream_config: StreamConfig = {
            #[cfg(target_os = "linux")]
            {
                Self::linux_stream_config(audio_device, config)
            }

            #[cfg(not(target_os = "linux"))]
            {
                config.clone().into()
            }
        };

        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => {
                let capture_clone = capture.clone();
                device.build_input_stream(
                    &stream_config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        capture.process_audio_data(data);
                    },
                    move |err| {
                        capture_clone.handle_stream_error(err);
                    },
                    None,
                )?
            }
            cpal::SampleFormat::I16 => {
                let capture_clone = capture.clone();
                device.build_input_stream(
                    &stream_config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        let f32_data: Vec<f32> = data
                            .iter()
                            .map(|&sample| sample as f32 / i16::MAX as f32)
                            .collect();
                        capture.process_audio_data(&f32_data);
                    },
                    move |err| {
                        capture_clone.handle_stream_error(err);
                    },
                    None,
                )?
            }
            cpal::SampleFormat::I32 => {
                let capture_clone = capture.clone();
                device.build_input_stream(
                    &stream_config,
                    move |data: &[i32], _: &cpal::InputCallbackInfo| {
                        let f32_data: Vec<f32> = data
                            .iter()
                            .map(|&sample| sample as f32 / i32::MAX as f32)
                            .collect();
                        capture.process_audio_data(&f32_data);
                    },
                    move |err| {
                        capture_clone.handle_stream_error(err);
                    },
                    None,
                )?
            }
            cpal::SampleFormat::I8 => {
                let capture_clone = capture.clone();
                device.build_input_stream(
                    &stream_config,
                    move |data: &[i8], _: &cpal::InputCallbackInfo| {
                        let f32_data: Vec<f32> = data
                            .iter()
                            .map(|&sample| sample as f32 / i8::MAX as f32)
                            .collect();
                        capture.process_audio_data(&f32_data);
                    },
                    move |err| {
                        capture_clone.handle_stream_error(err);
                    },
                    None,
                )?
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported sample format: {:?}",
                    config.sample_format()
                ));
            }
        };

        Ok(stream)
    }

    /// Get device info
    pub fn device(&self) -> &AudioDevice {
        &self.device
    }

    /// Stop the stream
    pub fn stop(self) -> Result<()> {
        info!("Stopping audio stream for device: {}", self.device.name);

        match self.backend {
            StreamBackend::Cpal(stream) => {
                // CRITICAL: Pause the stream first to stop callbacks immediately
                // This ensures closures stop executing before we drop the stream,
                // allowing Arc references captured in callbacks to be released
                if let Err(e) = stream.pause() {
                    warn!("Failed to pause stream before drop: {}", e);
                }
                info!("Stream paused, now dropping to release callbacks");
                drop(stream);
            }
            #[cfg(target_os = "macos")]
            StreamBackend::CoreAudio { task } => {
                // Abort the processing task and wait briefly for cleanup
                if let Some(task_handle) = task {
                    info!("Aborting Core Audio task...");
                    task_handle.abort();
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    info!("Core Audio task aborted");
                }
            }
            #[cfg(target_os = "linux")]
            StreamBackend::LinuxSystemAudio { task } => {
                if let Some(task_handle) = task {
                    info!("Aborting Linux system audio capture task...");
                    task_handle.abort();
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    info!("Linux system audio capture task aborted");
                }
            }
        }

        // Explicitly drop self.device Arc reference
        drop(self.device);
        info!("Audio stream stopped and device reference dropped");
        Ok(())
    }
}

/// Audio stream manager for handling multiple streams
pub struct AudioStreamManager {
    microphone_stream: Option<AudioStream>,
    system_stream: Option<AudioStream>,
    state: Arc<RecordingState>,
}

// SAFETY: AudioStreamManager contains AudioStream which we've marked as Send
unsafe impl Send for AudioStreamManager {}

impl AudioStreamManager {
    pub fn new(state: Arc<RecordingState>) -> Self {
        Self {
            microphone_stream: None,
            system_stream: None,
            state,
        }
    }

    /// Start audio streams for the given devices
    pub async fn start_streams(
        &mut self,
        microphone_device: Option<Arc<AudioDevice>>,
        system_device: Option<Arc<AudioDevice>>,
        recording_sender: Option<mpsc::UnboundedSender<super::recording_state::AudioChunk>>,
    ) -> Result<()> {
        use super::capture::get_current_backend;
        let backend = get_current_backend();
        info!("🎙️ Starting audio streams with backend: {:?}", backend);

        // Start microphone stream
        if let Some(mic_device) = microphone_device {
            info!(
                "🎤 Creating microphone stream: {} (always uses CPAL)",
                mic_device.name
            );
            match AudioStream::create(
                mic_device.clone(),
                self.state.clone(),
                DeviceType::Microphone,
                recording_sender.clone(),
            )
            .await
            {
                Ok(stream) => {
                    self.state.set_microphone_device(mic_device);
                    self.microphone_stream = Some(stream);
                    info!("✅ Microphone stream created successfully");
                }
                Err(e) => {
                    error!("❌ Failed to create microphone stream: {}", e);
                    return Err(e);
                }
            }
        } else {
            info!("ℹ️ No microphone device specified, skipping microphone stream");
        }

        // Start system audio stream
        if let Some(sys_device) = system_device {
            info!(
                "🔊 Creating system audio stream: {} (backend: {:?})",
                sys_device.name, backend
            );
            match AudioStream::create(
                sys_device.clone(),
                self.state.clone(),
                DeviceType::System,
                recording_sender.clone(),
            )
            .await
            {
                Ok(stream) => {
                    self.state.set_system_device(sys_device);
                    self.system_stream = Some(stream);
                    info!("✅ System audio stream created with {:?} backend", backend);
                }
                Err(e) => {
                    warn!("⚠️ Failed to create system audio stream: {}", e);
                    // Don't fail if only system audio fails
                }
            }
        } else {
            info!("ℹ️ No system device specified, skipping system audio stream");
        }

        // Ensure at least one stream was created
        if self.microphone_stream.is_none() && self.system_stream.is_none() {
            return Err(anyhow::anyhow!("No audio streams could be created"));
        }

        Ok(())
    }

    /// Stop all audio streams
    pub fn stop_streams(&mut self) -> Result<()> {
        info!("Stopping all audio streams");

        let mut errors = Vec::new();

        // Stop microphone stream
        if let Some(mic_stream) = self.microphone_stream.take() {
            if let Err(e) = mic_stream.stop() {
                error!("Failed to stop microphone stream: {}", e);
                errors.push(e);
            }
        }

        // Stop system stream
        if let Some(sys_stream) = self.system_stream.take() {
            if let Err(e) = sys_stream.stop() {
                error!("Failed to stop system stream: {}", e);
                errors.push(e);
            }
        }

        if !errors.is_empty() {
            Err(anyhow::anyhow!("Failed to stop some streams: {:?}", errors))
        } else {
            info!("All audio streams stopped successfully");
            Ok(())
        }
    }

    /// Get stream count
    pub fn active_stream_count(&self) -> usize {
        let mut count = 0;
        if self.microphone_stream.is_some() {
            count += 1;
        }
        if self.system_stream.is_some() {
            count += 1;
        }
        count
    }

    /// Check if any streams are active
    pub fn has_active_streams(&self) -> bool {
        self.microphone_stream.is_some() || self.system_stream.is_some()
    }
}

impl Drop for AudioStreamManager {
    fn drop(&mut self) {
        if let Err(e) = self.stop_streams() {
            error!("Error stopping streams during drop: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "linux")]
    fn linux_clamps_buffer_override_to_supported_range() {
        let supported = SupportedBufferSize::Range {
            min: 256,
            max: 2048,
        };

        assert_eq!(AudioStream::clamp_linux_buffer_frames(128, &supported), 256);
        assert_eq!(
            AudioStream::clamp_linux_buffer_frames(1024, &supported),
            1024
        );
        assert_eq!(
            AudioStream::clamp_linux_buffer_frames(4096, &supported),
            2048
        );
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn linux_preserves_requested_buffer_when_supported_range_is_unknown() {
        assert_eq!(
            AudioStream::clamp_linux_buffer_frames(4096, &SupportedBufferSize::Unknown),
            4096
        );
    }
}
