# Linux Mint Ralph-Loop Execution Pack

## Purpose
This document turns `docs/linux_mint_support_prd.md` into an execution-ready multi-agent plan.

Use this when coordinating parallel workstreams against the Meetily fork to make Linux Mint support real and releasable.

## Execution Goal
Deliver a Linux beta for:
- Linux Mint 22.x
- Linux Mint 21.x
- x86_64
- CPU fallback
- broad CUDA-capable NVIDIA acceleration
- `.deb` and `AppImage`, with `.deb` as the primary recommended install path
- microphone + system audio capture

## Parent Branch Strategy
- Integration branch: `epic/linux-mint-support`

Child branches:
- `fix/linux-build-baseline`
- `fix/linux-nvidia-path`
- `fix/linux-device-picker`
- `fix/linux-system-audio`
- `fix/linux-stream-stability`
- `docs/linux-beta-guide`

## Merge Order
1. `fix/linux-build-baseline`
2. `fix/linux-nvidia-path`
3. `fix/linux-device-picker`
4. `fix/linux-system-audio`
5. `fix/linux-stream-stability`
6. `docs/linux-beta-guide`
7. merge integration branch back to fork `main` when validated

## Non-Negotiable Guardrails
- Keep changes scoped to the assigned work package
- Do not silently refactor unrelated code
- Preserve cross-platform behavior for macOS and Windows
- Prefer feature flags, cfg-gating, and Linux-specific branches over broad shared-risk rewrites
- Every work package must leave exact test commands and exact files changed
- Report blockers instead of free-ranging into adjacent domains

## Shared Definition of Done
A work package is not done until it includes:
1. code changes or explicit no-change findings
2. exact files touched
3. exact commands run
4. observed result
5. open risks or follow-up items

## Parent Coordinator Prompt
```text
Coordinate Linux Mint support delivery for the Meetily fork using the PRD at `docs/linux_mint_support_prd.md` and this execution pack.

Primary outcome:
- Linux Mint 22.x and 21.x support
- x86_64 beta
- CPU fallback and NVIDIA acceleration
- AppImage and .deb packaging
- microphone plus system audio capture
- stable recording while common call apps are active

Create and supervise child tasks for:
1. build baseline and packaging
2. NVIDIA acceleration path
3. Linux device picker cleanup
4. Linux system audio capture
5. Linux stream stability
6. QA/docs/release notes

Requirements:
- Use integration branch `epic/linux-mint-support`
- Keep each child scoped to its package
- Require exact file paths, exact commands, and exact blockers
- Merge in the prescribed order
- Escalate conflicts between child changes instead of hand-waving them away
- Do not declare success without Linux-specific validation evidence
- Treat this as internal fork work for now, not a public upstream release push
```

## Child Work Package Prompts

### A. Build Baseline and Packaging
```text
Work on branch `fix/linux-build-baseline`.

Goal:
Make the Linux Mint build path reproducible for Mint 22.x and Mint 21.x, including local dev setup and fork CI artifact production.

Focus files:
- `.github/workflows/build-linux.yml`
- `docs/building_in_linux.md`
- `docs/BUILDING.md`
- `frontend/build-gpu.sh`
- `frontend/dev-gpu.sh`
- `frontend/src-tauri/tauri.conf.json`
- `frontend/src-tauri/build/*`

Key known issue:
- local Linux build/check requires the `llama-helper` sidecar to exist in `frontend/src-tauri/binaries/`

Tasks:
- remove sidecar build/copy friction
- verify clean local build path from fresh checkout
- verify fork CI can emit `.deb` and `AppImage`
- ensure docs match the real workflow

Do not change deep audio logic unless strictly required to unblock packaging.
```

### B. NVIDIA Acceleration Path
```text
Work on branch `fix/linux-nvidia-path`.

Goal:
Make the Linux NVIDIA build path real and documented for Mint 22.x and Mint 21.x, while preserving CPU fallback.

Focus files:
- `frontend/build-gpu.sh`
- `frontend/dev-gpu.sh`
- `docs/building_in_linux.md`
- `docs/GPU_ACCELERATION.md`
- relevant Rust build/config files only if needed

Tasks:
- verify current NVIDIA detection assumptions
- verify required toolchain/env for CUDA builds
- fix build-script or docs mismatches
- keep CPU fallback intact
- leave AMD/ROCm issues out of scope unless they block shared Linux logic

Do not take ownership of PipeWire audio behavior.
```

### C. Linux Device Picker Cleanup
```text
Work on branch `fix/linux-device-picker`.

Goal:
Make Linux audio device selection understandable and usable on Mint.

Focus files:
- `frontend/src-tauri/src/audio/devices/platform/linux.rs`
- `frontend/src-tauri/src/audio/devices/discovery.rs`
- `frontend/src-tauri/src/audio/devices/configuration.rs`
- `frontend/src-tauri/src/audio/devices/microphone.rs`
- `frontend/src-tauri/src/audio/devices/speakers.rs`

Known problem:
- raw ALSA names like `sysdefault:CARD=`, `front:CARD=`, `surround*`, `iec958`, `hw:` flood the picker

Tasks:
- keep logical endpoints such as `default`, `pipewire`, `pulse`, `jack`, and other actually-usable entries
- prevent fallback enumeration from re-adding filtered junk
- keep system-audio-related endpoints discoverable
- improve default selection behavior on Linux where possible

Do not rewrite capture logic beyond what is needed for correct device identity.
```

### D. Linux System Audio Capture
```text
Work on branch `fix/linux-system-audio`.

Goal:
Make Linux system audio capture actually work on Mint using common PipeWire or PulseAudio setups.

Focus files:
- `frontend/src-tauri/src/audio/devices/platform/linux.rs`
- `frontend/src-tauri/src/audio/devices/configuration.rs`
- `frontend/src-tauri/src/audio/recording_manager.rs`
- `frontend/src-tauri/src/audio/recording_commands.rs`
- `frontend/src-tauri/src/audio/stream.rs`
- `frontend/src-tauri/src/audio/capture/system.rs`

Known tension to investigate:
- some Linux code tries to use monitor sources through CPAL
- `capture/system.rs` still says non-macOS system audio is not implemented

Tasks:
- identify the real active Linux capture path
- remove or isolate stale/conflicting Linux system-audio code
- make monitor-source capture work for real Mint setups
- ensure transcript pipeline can see captured system audio
- fail gracefully to mic-only when loopback is unavailable

Do not take ownership of CI packaging unless needed for local validation.
```

### E. Linux Stream Stability
```text
Work on branch `fix/linux-stream-stability`.

Goal:
Prevent Meetily from degrading active Teams, Zoom, Discord, or browser WebRTC audio on Linux while recording.

Focus files:
- `frontend/src-tauri/src/audio/stream.rs`
- nearby Linux-specific stream/config helpers if required

Known issue:
- CPAL/ALSA path may be causing too-small buffer requests that shrink PipeWire quantum globally

Tasks:
- verify current Linux buffer-size behavior
- clamp or set more stable Linux buffer sizing if needed
- validate no severe choppiness or A/V desync in concurrent apps
- keep latency acceptable for transcription use

Do not redesign the whole audio pipeline.
```

### F. QA, Docs, and Release Readiness
```text
Work on branch `docs/linux-beta-guide`.

Goal:
Turn the integrated Linux work into something another Mint tester can actually use.

Focus files:
- `README.md`
- `docs/building_in_linux.md`
- `docs/BUILDING.md`
- `docs/linux_mint_support_prd.md`
- new Linux QA/smoke-test docs as needed

Tasks:
- produce Mint 22 and Mint 21 smoke-test checklist
- document CPU fallback and NVIDIA setup
- document AppImage and .deb install/test flow
- list known Linux limitations and troubleshooting steps
- align public-facing repo messaging with actual Linux support state

Do not own core Rust fixes except tiny doc-driven corrections.
```

## Recommended Parallelization
Safe to run in parallel first:
- A. Build baseline and packaging
- B. NVIDIA acceleration path
- C. Linux device picker cleanup
- D. Linux system audio capture investigation

Run after D starts stabilizing:
- E. Linux stream stability

Run continuously / near the end:
- F. QA, docs, release readiness

## Integration Checklist
Before merging into `epic/linux-mint-support`, verify:
- branch rebased or merged cleanly
- exact commands rerun after merge
- no Linux-only fix broke macOS or Windows cfg sections
- docs updated if workflow changed
- blocker notes preserved if unresolved

## Validation Checklist for Final Beta
- builds on Mint 22.x
- builds on Mint 21.x
- local dev run works
- `.deb` artifact works
- `AppImage` artifact works
- mic capture works
- system audio capture works
- transcript path works with system audio present
- summary path still works
- concurrent Teams/Zoom/Discord/browser call remains stable
- CPU fallback works
- NVIDIA path works

## Additional Locked Execution Decisions
- Prefer `.deb` in first beta guidance and release notes, while also shipping `AppImage`
- Aim for a broad CUDA-capable NVIDIA range first, then tighten only if validation proves we must
- Keep planning and tracking internal on the fork for now
