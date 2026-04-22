#!/bin/bash
# GPU-accelerated build script for Meetily
# Automatically detects and builds with optimal GPU features

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FRONTEND_DIR="$SCRIPT_DIR"

cd "$FRONTEND_DIR"

echo -e "${BLUE}🚀 Meetily GPU-Accelerated Build Script${NC}"
echo ""

# Detect OS
if [[ "$OSTYPE" == "darwin"* ]]; then
  OS="macos"
elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
  OS="linux"
else
  echo -e "${RED}❌ Unsupported OS: $OSTYPE${NC}"
  exit 1
fi

# Function to check if command exists
command_exists() {
  command -v "$1" >/dev/null 2>&1
}

echo ""
echo -e "${BLUE}📦 Building Meetily...${NC}"
echo ""

# Check for pnpm or npm
if command_exists pnpm; then
  PKG_MGR="pnpm"
elif command_exists npm; then
  PKG_MGR="npm"
else
  echo -e "${RED}❌ Neither npm nor pnpm found${NC}"
  exit 1
fi

# Detect GPU feature if not already set
feature_overridden=false
if [ -z "${TAURI_GPU_FEATURE+x}" ]; then
    echo -e "${BLUE}🔍 Detecting GPU features...${NC}"
    TAURI_GPU_FEATURE=$(node scripts/auto-detect-gpu.js)
else
    feature_overridden=true
fi

if [ -n "${TAURI_GPU_FEATURE:-}" ] && [ "$TAURI_GPU_FEATURE" != "none" ]; then
    echo -e "${GREEN}✅ Detected GPU feature: $TAURI_GPU_FEATURE${NC}"
elif [ "$feature_overridden" = true ] || [ "${TAURI_GPU_FEATURE:-}" = "none" ]; then
    echo -e "${YELLOW}⚠️ CPU-only mode requested explicitly${NC}"
else
    echo -e "${YELLOW}⚠️ No specific GPU feature detected or forced. Building in CPU-only mode.${NC}"
fi

export TAURI_GPU_FEATURE

echo ""
echo -e "${BLUE}🦙 Preparing llama-helper sidecar (release)...${NC}"
"$FRONTEND_DIR/scripts/prepare-llama-helper-sidecar.sh" release

# Build using npm scripts
echo -e "${BLUE}Building complete Tauri application...${NC}"
echo ""

# NO_STRIP true due to issues with bundling AppImage
NO_STRIP=true $PKG_MGR run tauri:build

echo ""
echo -e "${GREEN}✅ Build completed successfully!${NC}"
echo ""
echo -e "${GREEN}🎉 Complete Tauri application built with GPU acceleration!${NC}"

