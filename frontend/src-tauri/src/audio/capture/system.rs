use std::pin::Pin;
use std::task::{Context, Poll};

use anyhow::Result;
use futures_util::{Stream, StreamExt};

#[cfg(any(target_os = "macos", target_os = "linux"))]
use futures_channel::mpsc;

#[cfg(target_os = "macos")]
use super::core_audio::CoreAudioCapture;
#[cfg(target_os = "macos")]
use cpal::traits::{DeviceTrait, HostTrait};
#[cfg(target_os = "macos")]
use log::info;

#[cfg(target_os = "linux")]
use log::{debug, info, warn};
#[cfg(target_os = "linux")]
use std::sync::{Arc, Mutex};
#[cfg(target_os = "linux")]
use tokio::io::AsyncReadExt;
#[cfg(target_os = "linux")]
use tokio::process::{Child, Command};

/// System audio capture using Core Audio tap (macOS) or PulseAudio/PipeWire monitor sources (Linux)
pub struct SystemAudioCapture {
    #[cfg(target_os = "macos")]
    _host: cpal::Host,
}

impl SystemAudioCapture {
    pub fn new() -> Result<Self> {
        #[cfg(target_os = "macos")]
        {
            let host = cpal::default_host();
            return Ok(Self { _host: host });
        }

        #[cfg(not(target_os = "macos"))]
        {
            Ok(Self {})
        }
    }

    pub fn list_system_devices() -> Result<Vec<String>> {
        #[cfg(target_os = "macos")]
        {
            let host = cpal::default_host();
            let devices = host
                .output_devices()
                .map_err(|e| anyhow::anyhow!("Failed to enumerate output devices: {}", e))?;

            let mut device_names = Vec::new();
            for device in devices {
                if let Ok(name) = device.name() {
                    device_names.push(name);
                }
            }

            Ok(device_names)
        }

        #[cfg(target_os = "linux")]
        {
            Ok(
                crate::audio::devices::platform::linux::list_linux_system_audio_devices()?
                    .into_iter()
                    .map(|device| device.name)
                    .collect(),
            )
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            Ok(Vec::new())
        }
    }

    pub fn start_system_audio_capture(&self) -> Result<SystemAudioStream> {
        self.start_system_audio_capture_for_device(None)
    }

    pub fn start_system_audio_capture_for_device(
        &self,
        device_name: Option<&str>,
    ) -> Result<SystemAudioStream> {
        #[cfg(target_os = "macos")]
        {
            info!("Starting Core Audio system capture (macOS)");
            let core_audio = CoreAudioCapture::new()?;
            let core_audio_stream = core_audio.stream()?;
            let sample_rate = core_audio_stream.sample_rate();

            let (tx, rx) = mpsc::unbounded::<Vec<f32>>();
            let (drop_tx, drop_rx) = std::sync::mpsc::channel::<()>();

            tokio::spawn(async move {
                use futures_util::StreamExt;
                let mut stream = core_audio_stream;
                let mut buffer = Vec::new();
                let chunk_size = 1024;

                loop {
                    if drop_rx.try_recv().is_ok() {
                        break;
                    }

                    match stream.next().await {
                        Some(sample) => {
                            buffer.push(sample);
                            if buffer.len() >= chunk_size {
                                if tx.unbounded_send(buffer.clone()).is_err() {
                                    break;
                                }
                                buffer.clear();
                            }
                        }
                        None => break,
                    }
                }

                if !buffer.is_empty() {
                    let _ = tx.unbounded_send(buffer);
                }
            });

            let receiver = rx.map(futures_util::stream::iter).flatten();

            info!("Core Audio system capture started successfully");

            Ok(SystemAudioStream {
                #[cfg(any(target_os = "macos", target_os = "linux"))]
                drop_tx,
                sample_rate,
                receiver: Box::pin(receiver),
                #[cfg(target_os = "linux")]
                child: None,
            })
        }

        #[cfg(target_os = "linux")]
        {
            let selected_device = match device_name {
                Some(name) => {
                    crate::audio::devices::platform::linux::resolve_linux_system_audio_device(name)?
                }
                None => {
                    crate::audio::devices::platform::linux::default_linux_system_audio_device()?
                }
            };

            let source_name = selected_device.name;
            info!(
                "Starting Linux system audio capture from PulseAudio/PipeWire monitor source: {}",
                source_name
            );

            let mut child = Command::new("parec")
                .args([
                    "--device",
                    &source_name,
                    "--format=float32le",
                    "--channels=2",
                    "--rate=48000",
                    "--raw",
                    "--latency-msec=50",
                    "--process-time-msec=50",
                ])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| {
                    anyhow::anyhow!(
                        "Failed to start parec for Linux system audio source '{}': {}",
                        source_name,
                        e
                    )
                })?;

            let stdout = child.stdout.take().ok_or_else(|| {
                anyhow::anyhow!("parec did not provide a stdout pipe for system audio capture")
            })?;
            let stderr = child.stderr.take();

            let child = Arc::new(Mutex::new(Some(child)));
            let child_for_task = Arc::clone(&child);

            let (tx, rx) = mpsc::unbounded::<Vec<f32>>();
            let (drop_tx, drop_rx) = std::sync::mpsc::channel::<()>();
            let source_name_for_task = source_name.clone();

            tokio::spawn(async move {
                let mut stdout = stdout;
                let mut buffer = vec![0_u8; 8192];

                if let Some(stderr) = stderr {
                    let source_name_for_stderr = source_name_for_task.clone();
                    tokio::spawn(async move {
                        let mut stderr = stderr;
                        let mut stderr_bytes = Vec::new();
                        match stderr.read_to_end(&mut stderr_bytes).await {
                            Ok(_) if !stderr_bytes.is_empty() => {
                                debug!(
                                    "parec stderr for '{}': {}",
                                    source_name_for_stderr,
                                    String::from_utf8_lossy(&stderr_bytes).trim()
                                );
                            }
                            Ok(_) => {}
                            Err(e) => warn!(
                                "Failed reading parec stderr for '{}': {}",
                                source_name_for_stderr, e
                            ),
                        }
                    });
                }

                loop {
                    if drop_rx.try_recv().is_ok() {
                        break;
                    }

                    match stdout.read(&mut buffer).await {
                        Ok(0) => break,
                        Ok(bytes_read) => {
                            let mut samples = Vec::with_capacity(bytes_read / 4);
                            for chunk in buffer[..bytes_read].chunks_exact(4) {
                                samples.push(f32::from_le_bytes([
                                    chunk[0], chunk[1], chunk[2], chunk[3],
                                ]));
                            }

                            if !samples.is_empty() && tx.unbounded_send(samples).is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            warn!(
                                "Linux system audio capture read failed for '{}': {}",
                                source_name_for_task, e
                            );
                            break;
                        }
                    }
                }

                if let Ok(mut child_guard) = child_for_task.lock() {
                    if let Some(mut child) = child_guard.take() {
                        match child.start_kill() {
                            Ok(_) => debug!("Stopped parec capture for '{}'", source_name_for_task),
                            Err(e) => debug!(
                                "parec capture for '{}' already stopped or could not be killed cleanly: {}",
                                source_name_for_task,
                                e
                            ),
                        }
                    }
                }
            });

            let receiver = rx.map(futures_util::stream::iter).flatten();

            Ok(SystemAudioStream {
                drop_tx,
                sample_rate: 48_000,
                receiver: Box::pin(receiver),
                child: Some(child),
            })
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            let _ = device_name;
            anyhow::bail!("System audio capture not yet implemented for this platform")
        }
    }

    pub fn check_system_audio_permissions() -> bool {
        #[cfg(target_os = "linux")]
        {
            crate::audio::devices::platform::linux::linux_system_audio_available()
        }

        #[cfg(not(target_os = "linux"))]
        {
            match cpal::default_host().output_devices() {
                Ok(_) => true,
                Err(_) => false,
            }
        }
    }
}

pub struct SystemAudioStream {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    drop_tx: std::sync::mpsc::Sender<()>,
    sample_rate: u32,
    receiver: Pin<Box<dyn Stream<Item = f32> + Send + Sync>>,
    #[cfg(target_os = "linux")]
    child: Option<Arc<Mutex<Option<Child>>>>,
}

impl Drop for SystemAudioStream {
    fn drop(&mut self) {
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        let _ = self.drop_tx.send(());

        #[cfg(target_os = "linux")]
        {
            if let Some(child) = self.child.take() {
                if let Ok(mut child_guard) = child.lock() {
                    if let Some(mut child) = child_guard.take() {
                        let _ = child.start_kill();
                    }
                }
            }
        }
    }
}

impl Stream for SystemAudioStream {
    type Item = f32;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.receiver.as_mut().poll_next_unpin(cx)
    }
}

impl SystemAudioStream {
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}

/// Public interface for system audio capture
pub async fn start_system_audio_capture() -> Result<SystemAudioStream> {
    let capture = SystemAudioCapture::new()?;
    capture.start_system_audio_capture()
}

pub fn list_system_audio_devices() -> Result<Vec<String>> {
    SystemAudioCapture::list_system_devices()
}

pub fn check_system_audio_permissions() -> bool {
    SystemAudioCapture::check_system_audio_permissions()
}
