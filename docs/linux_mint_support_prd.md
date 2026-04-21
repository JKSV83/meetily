# Linux Mint Support PRD (Draft v0.1)

## Status
Draft for planning and multi-agent execution.

## Objective
Make Meetily build, launch, record, transcribe, and package reliably on Linux Mint, starting with Mint 22.x and then validating Mint 21.x.

## Why This Work Matters
Meetily already has partial Linux support in source, docs, and CI, but Linux is not release-ready today.

What we already verified:
- Linux build docs exist in `docs/building_in_linux.md` and `docs/BUILDING.md`
- Linux CI exists in `.github/workflows/build-linux.yml`
- Linux bundle targets exist in `frontend/src-tauri/tauri.conf.json`
- Current source can pass `cargo check -p meetily` on Linux once the `llama-helper` sidecar is built and copied into `frontend/src-tauri/binaries/`
- Latest public release still ships macOS and Windows assets only

## Product Goal
Ship a Linux Mint beta that is good enough for real daily use on x86_64, not just a docs-only or CI-only Linux story.

## Primary Target
- Linux Mint 22.x (Ubuntu 24.04 base)

## Secondary Target
- Linux Mint 21.x (Ubuntu 22.04 base)

## Phase 1 Success Criteria
A build is considered successful when all of the following are true on Mint 22.x:

1. Developer setup works from a clean machine using documented commands
2. App builds locally without manual mystery steps
3. Fork CI can produce Linux artifacts
4. App launches successfully
5. Microphone capture works
6. System audio capture works on PipeWire or PulseAudio setups commonly seen on Mint
7. Device picker is usable and does not flood users with raw ALSA profile names
8. Starting Meetily does not glitch active Teams, Zoom, Discord, or browser WebRTC audio
9. Basic transcription and summary flow still works end to end
10. We have a Linux troubleshooting guide and smoke-test checklist

## Explicit Non-Goals for Phase 1
- Flatpak support
- ARM Linux support
- Perfect AMD ROCm support on day one
- Broad distro parity beyond Mint 22 / Mint 21 / close Ubuntu equivalents
- Solving every Linux desktop environment edge case before beta

## Current Known Gaps

### 1. Linux build path is real, but dev ergonomics are fragile
Observed issue:
- `cargo check -p meetily` initially failed because `frontend/src-tauri/binaries/llama-helper-x86_64-unknown-linux-gnu` was missing
- After building `llama-helper` and copying it into `frontend/src-tauri/binaries/`, the Linux check passed

Meaning:
- Linux is not blocked at the architecture level
- Local build flow needs cleanup and standardization

### 2. System audio on Linux is the biggest risk
Relevant code and evidence:
- `frontend/src-tauri/src/audio/devices/platform/linux.rs`
- `frontend/src-tauri/src/audio/devices/discovery.rs`
- `frontend/src-tauri/src/audio/devices/configuration.rs`
- `frontend/src-tauri/src/audio/stream.rs`
- `frontend/src-tauri/src/audio/capture/system.rs`

Known reports:
- `#383` system sound not captured on PipeWire
- `#433` Meetily recording causes concurrent app audio glitches on Linux
- `#437` device picker shows raw ALSA names and is effectively unusable
- `#305` AMD GPU Linux build issues

### 3. Linux audio architecture appears partially implemented and partially duplicated
Observed issues:
- `capture/system.rs` still says non-macOS system audio capture is not yet implemented
- Other Linux code paths attempt loopback through monitor sources and CPAL
- This suggests stale or split logic that should be clarified before deeper fixes

## Proposed Delivery Strategy
Use a staged approach instead of attempting all Linux concerns at once.

### Milestone 1: Buildable Linux Developer Preview
Goal:
- Clean local Mint build and launch
- CPU path first
- Mic recording works
- Packaging path is reliable enough for internal testing

### Milestone 2: Linux Audio Beta
Goal:
- PipeWire and PulseAudio system audio capture works
- Device picker is usable
- Concurrent call apps remain stable while Meetily records

### Milestone 3: Fork Release Candidate
Goal:
- AppImage and `.deb` artifacts published from fork CI
- Smoke-tested on Mint 22 and Mint 21
- Linux docs updated

## Priority Order

### P0. Build and Packaging Baseline
Tackle first.

Scope:
- Make Linux dev build reproducible from a clean repo checkout
- Standardize sidecar build and copy behavior
- Validate `.deb` and `AppImage` generation from fork CI
- Confirm docs match reality

Primary files:
- `.github/workflows/build-linux.yml`
- `docs/building_in_linux.md`
- `docs/BUILDING.md`
- `frontend/build-gpu.sh`
- `frontend/dev-gpu.sh`
- `frontend/src-tauri/tauri.conf.json`
- `frontend/src-tauri/build/*`

Definition of done:
- One documented path works on Mint 22 without ad hoc fixes
- CI artifacts build on fork

### P1. Linux Audio Device Discovery and UX
Tackle second, in parallel with capture investigation if needed.

Scope:
- Filter noisy ALSA device aliases
- Preserve useful logical endpoints like `default`, `pipewire`, `pulse`
- Prevent discovery fallback from re-adding junk entries
- Ensure default device selection chooses sensible Linux endpoints

Primary files:
- `frontend/src-tauri/src/audio/devices/platform/linux.rs`
- `frontend/src-tauri/src/audio/devices/discovery.rs`
- `frontend/src-tauri/src/audio/devices/configuration.rs`
- `frontend/src-tauri/src/audio/devices/microphone.rs`
- `frontend/src-tauri/src/audio/devices/speakers.rs`

Definition of done:
- Device pickers show a short, usable list on Mint
- System audio source selection is understandable

### P2. PipeWire / PulseAudio System Audio Capture
Tackle as the core Linux feature.

Scope:
- Validate the active Linux system-audio path
- Remove or quarantine stale code paths that confuse behavior
- Ensure monitor-source capture works for common Mint setups
- Fail gracefully to mic-only mode when system audio is unavailable

Primary files:
- `frontend/src-tauri/src/audio/devices/platform/linux.rs`
- `frontend/src-tauri/src/audio/devices/configuration.rs`
- `frontend/src-tauri/src/audio/recording_manager.rs`
- `frontend/src-tauri/src/audio/recording_commands.rs`
- `frontend/src-tauri/src/audio/stream.rs`
- `frontend/src-tauri/src/audio/capture/system.rs`

Definition of done:
- System audio is actually recorded and transcribed on Mint test machines

### P3. Runtime Stability While Other Call Apps Are Running
High-value Linux fix.

Scope:
- Address PipeWire quantum shrink / small buffer problems
- Explicitly set or clamp buffer sizes on Linux
- Verify Teams, Zoom, Discord, browser WebRTC are not degraded while Meetily records

Primary files:
- `frontend/src-tauri/src/audio/stream.rs`
- any Linux-specific stream/config helpers discovered during implementation

Definition of done:
- No reproducible severe choppiness or A/V desync in another active app during recording

### P4. Linux Release and Documentation Polish
After core functionality is stable.

Scope:
- Publish Linux beta artifacts from fork
- Update README and Linux docs
- Add smoke-test checklist and known-issues section
- Align repo messaging with actual support state

Primary files:
- `README.md`
- `docs/building_in_linux.md`
- `.github/workflows/build-linux.yml`
- release metadata as needed

Definition of done:
- New tester can install a Linux beta and follow docs without hidden tribal knowledge

## Multi-Agent Execution Plan
Recommended integration branch:
- `epic/linux-mint-support`

Recommended child branches:
- `fix/linux-build-baseline`
- `fix/linux-device-picker`
- `fix/linux-system-audio`
- `fix/linux-stream-stability`
- `docs/linux-beta-guide`

### Agent Work Package A: Build and Packaging Baseline
Mission:
- Reproduce clean Linux builds and remove the sidecar/dev-flow friction

Boundaries:
- Build scripts, CI, tauri packaging, docs
- Avoid deep audio logic changes

Expected outputs:
- Reliable local build flow
- Reliable fork CI Linux artifacts
- Updated Linux build instructions

### Agent Work Package B: Linux Device Discovery and Picker Cleanup
Mission:
- Make audio device enumeration sane on Mint

Boundaries:
- Device discovery and filtering only
- Avoid changing transcription or recording pipeline logic unless required for selection correctness

Expected outputs:
- Filtered device lists
- Better defaults
- No raw ALSA flood in picker UI

### Agent Work Package C: Linux System Audio Capture Path
Mission:
- Make system audio loopback actually work on Mint

Boundaries:
- Linux capture path, monitor-source mapping, command path alignment
- Avoid packaging or release work

Expected outputs:
- Recorded system audio present in output
- Transcription sees system-audio speech when expected
- Graceful fallback when loopback not available

### Agent Work Package D: Linux Stream Stability
Mission:
- Stop Meetily from degrading active call apps on Linux

Boundaries:
- Buffer sizing, stream configuration, runtime behavior under concurrent load

Expected outputs:
- Stable concurrent Teams/Zoom/WebRTC behavior
- Linux-specific stream tuning if needed

### Agent Work Package E: QA, Docs, and Release Readiness
Mission:
- Validate the integrated result and make it easy to test

Boundaries:
- Smoke tests, docs, troubleshooting, artifact checks
- Avoid introducing new core logic unless fixing minor issues discovered during validation

Expected outputs:
- Linux smoke-test matrix
- Beta test notes
- Updated docs

## Recommended Merge Order
1. Build baseline
2. Device picker cleanup
3. System audio capture
4. Stream stability
5. Docs and release polish

Reason:
- Build reliability unblocks everyone
- Cleaner device discovery makes system-audio work easier to debug
- Stability tuning should land after the main capture path is confirmed

## Test Matrix

### OS Matrix
- Mint 22.x Cinnamon, x86_64
- Mint 21.x Cinnamon, x86_64

### Audio Matrix
- PipeWire + pipewire-pulse
- PulseAudio fallback if available
- Wired mic
- USB mic
- Bluetooth headset optional, not required for Phase 1 success

### Packaging Matrix
- Local dev run
- `.deb`
- `AppImage`

### Functional Matrix
- App launch
- Mic-only recording
- Mic + system audio recording
- Transcript generation
- Summary generation
- Export / save behavior

### Stress Matrix
- Record while Teams/Zoom/Discord/browser call is active
- Record while music or video plays through system output

## Risks
- PipeWire and CPAL behavior may require Linux-specific tuning beyond small patching
- Some current Linux code may be stale, split, or partially dead
- AMD GPU acceleration may remain out of scope for first beta
- Packaging success in CI does not guarantee runtime success on Mint without smoke testing

## Locked Product Decisions
1. First beta target includes both Mint 22.x and Mint 21.x
2. Optimize for generalized Mint support, not a single machine-specific configuration
3. System audio capture is required for the first usable beta
4. First beta should ship both `AppImage` and `.deb`
5. NVIDIA acceleration is in scope for Phase 1, alongside CPU fallback
6. First public-facing beta guidance should lead with the `.deb`, while still shipping `AppImage`
7. Aim for a broad CUDA-capable NVIDIA range in Phase 1, then narrow to the tested floor if validation forces it
8. Track the work internally on the fork for now, not as a public upstream campaign

## Execution Implications of Those Decisions
- Mint 22 and Mint 21 both need to stay in the validation matrix from the start
- Linux CI and docs should explicitly cover both Ubuntu 24.04-equivalent and Ubuntu 22.04-equivalent paths
- Build-baseline work must include CPU fallback and NVIDIA acceleration validation
- System-audio work cannot be deferred behind a mic-only preview unless the whole beta scope is explicitly reduced later
- Packaging must verify both `AppImage` and `.deb`, not just one format
- QA and docs should present `.deb` as the primary install path and `AppImage` as the fallback/portable option
- NVIDIA work should avoid arbitrarily narrowing support unless the toolchain or runtime proves a hard compatibility floor
- Project management can stay branch-and-doc driven on the fork without needing public labels or milestones yet

## Suggested First Ralph-Loop Prompt
Create and coordinate child tasks to deliver Milestone 1 and Milestone 2 for Linux Mint support in the Meetily fork. The first beta target must support both Mint 22.x and Mint 21.x, generalized Mint environments, system audio capture, both `AppImage` and `.deb` packaging with `.deb` as the primary recommended install path, and broad CUDA-capable NVIDIA support plus CPU fallback. Prioritize build reliability, Linux audio device enumeration, working PipeWire/PulseAudio system audio capture, concurrent-call stability, and reproducible validation across both Mint bases. Keep changes scoped by work package, use the integration branch `epic/linux-mint-support`, and report blockers with exact files and reproducible commands. Treat this as internal fork work for now, not a public upstream release push.
