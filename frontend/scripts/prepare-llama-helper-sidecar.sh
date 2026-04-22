#!/bin/bash
# Build and stage the llama-helper sidecar for the current host target.
# Usage: ./scripts/prepare-llama-helper-sidecar.sh [debug|release]

set -euo pipefail

PROFILE="${1:-debug}"
case "$PROFILE" in
  debug|release) ;;
  *)
    echo "Usage: $0 [debug|release]" >&2
    exit 1
    ;;
esac

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FRONTEND_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="$(cd "$FRONTEND_DIR/.." && pwd)"
HELPER_DIR="$REPO_ROOT/llama-helper"
TARGET_TRIPLE="$(rustc -vV | awk '/^host: / { print $2 }')"

if [ -z "$TARGET_TRIPLE" ]; then
  echo "❌ Failed to detect Rust host target triple" >&2
  exit 1
fi

BINARY_NAME="llama-helper"
SIDECAR_NAME="llama-helper-$TARGET_TRIPLE"
if [[ "$TARGET_TRIPLE" == *windows* ]]; then
  BINARY_NAME="llama-helper.exe"
  SIDECAR_NAME="llama-helper-$TARGET_TRIPLE.exe"
fi

FEATURE="${TAURI_GPU_FEATURE:-}"
if [ "$FEATURE" = "none" ]; then
  FEATURE=""
fi
if [ "$FEATURE" = "coreml" ]; then
  echo "ℹ️  Mapping TAURI_GPU_FEATURE=coreml to llama-helper feature metal"
  FEATURE="metal"
fi

CARGO_ARGS=()
if [ "$PROFILE" = "release" ]; then
  CARGO_ARGS+=(--release)
fi
if [ -n "$FEATURE" ]; then
  CARGO_ARGS+=(--features "$FEATURE")
fi

BINARIES_DIR="$FRONTEND_DIR/src-tauri/binaries"
BUILD_OUTPUT_DIR="$REPO_ROOT/target/$PROFILE"
SOURCE_BINARY="$BUILD_OUTPUT_DIR/$BINARY_NAME"
DEST_BINARY="$BINARIES_DIR/$SIDECAR_NAME"

mkdir -p "$BINARIES_DIR"

echo "🦙 Building llama-helper ($PROFILE) for $TARGET_TRIPLE${FEATURE:+ with feature $FEATURE}"
(
  cd "$HELPER_DIR"
  cargo build "${CARGO_ARGS[@]}"
)

if [ ! -f "$SOURCE_BINARY" ]; then
  echo "❌ Built llama-helper binary not found at $SOURCE_BINARY" >&2
  exit 1
fi

rm -f "$DEST_BINARY"
cp "$SOURCE_BINARY" "$DEST_BINARY"
chmod +x "$DEST_BINARY" 2>/dev/null || true

echo "✅ Staged llama-helper sidecar at $DEST_BINARY"
