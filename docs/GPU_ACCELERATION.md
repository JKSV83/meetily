# GPU Acceleration Guide

Meetily supports GPU acceleration for transcription, with CPU fallback preserved for every platform.

## Supported Backends

- CUDA, for NVIDIA GPUs
- Metal, for Apple Silicon and Macs with Metal support
- Core ML, for Apple Silicon
- Vulkan, for cross-platform GPU support
- OpenBLAS, for CPU optimization

## Linux Mint Notes

On Mint 22.x and Mint 21.x, the NVIDIA build path is real, but it still needs the CUDA toolkit, not just the driver.

Required for CUDA builds:
- `nvidia-smi` (driver present)
- `nvcc` or `CUDA_PATH` (toolkit present)
- `CMAKE_CUDA_ARCHITECTURES` only if you want to override the detected target arch list

If CUDA is not available, the build falls back to CPU mode.

## Automatic Detection

The Linux build scripts use this flow:

1. `scripts/auto-detect-gpu.js` chooses the feature flag
2. `scripts/tauri-auto.js` passes that feature to Tauri
3. If CUDA is selected on Linux, `CMAKE_CUDA_ARCHITECTURES` is resolved in this order:
   - existing `CMAKE_CUDA_ARCHITECTURES`
   - `CUDA_ARCHITECTURES`
   - detected NVIDIA compute caps from `nvidia-smi`
   - broad fallback list: `61;70;75;80;86;89;90`

That keeps one script usable across a broad set of CUDA-capable NVIDIA cards.

## Manual Overrides

```bash
# Force CUDA
TAURI_GPU_FEATURE=cuda ./dev-gpu.sh
TAURI_GPU_FEATURE=cuda ./build-gpu.sh

# Pin a CUDA arch list
CMAKE_CUDA_ARCHITECTURES=86 ./build-gpu.sh

# Force CPU-only
TAURI_GPU_FEATURE="" ./dev-gpu.sh
TAURI_GPU_FEATURE="" ./build-gpu.sh
```

## Linux Requirements Summary

- CUDA build: NVIDIA driver + CUDA toolkit
- Vulkan build: Vulkan SDK + BLAS headers
- CPU fallback: works without GPU SDKs

## Troubleshooting

- If `nvidia-smi` works but CUDA does not, install the toolkit.
- If auto-detection picks CPU mode, check that `nvcc` is on `PATH` or `CUDA_PATH` is set.
- If you want a specific build target, set `CMAKE_CUDA_ARCHITECTURES` explicitly.
