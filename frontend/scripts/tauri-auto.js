#!/usr/bin/env node
/**
 * Auto-detect GPU and run Tauri with appropriate features
 */

const { execSync } = require('child_process');
const path = require('path');
const fs = require('fs');
const os = require('os');

const frontendDir = path.resolve(__dirname, '..');
const repoRoot = path.resolve(frontendDir, '..');

function run(command, options = {}) {
  execSync(command, {
    stdio: 'inherit',
    ...options,
  });
}

function getHostTargetTriple() {
  return execSync('rustc -vV', {
    encoding: 'utf8',
    stdio: ['pipe', 'pipe', 'inherit'],
  })
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find((line) => line.startsWith('host: '))
    ?.replace(/^host:\s+/, '');
}

function normalizeHelperFeature(feature) {
  if (!feature || feature === 'none') {
    return '';
  }

  if (feature === 'coreml') {
    console.log('ℹ️  Mapping TAURI_GPU_FEATURE=coreml to llama-helper feature metal');
    return 'metal';
  }

  return feature;
}

function prepareLlamaHelperSidecar(command, env) {
  const profile = command === 'build' ? 'release' : 'debug';
  const helperDir = path.join(repoRoot, 'llama-helper');
  const binariesDir = path.join(frontendDir, 'src-tauri', 'binaries');

  if (!fs.existsSync(helperDir)) {
    throw new Error(`Could not find llama-helper directory at ${helperDir}`);
  }

  const targetTriple = getHostTargetTriple();
  if (!targetTriple) {
    throw new Error('Failed to detect Rust host target triple');
  }

  const helperFeature = normalizeHelperFeature(env.TAURI_GPU_FEATURE);
  const cargoArgs = ['build'];
  if (profile === 'release') {
    cargoArgs.push('--release');
  }
  if (helperFeature) {
    cargoArgs.push('--features', helperFeature);
  }

  console.log(`🦙 Preparing llama-helper sidecar (${profile}) for ${targetTriple}${helperFeature ? ` with feature ${helperFeature}` : ''}`);
  run(`cargo ${cargoArgs.join(' ')}`, { cwd: helperDir, env });

  fs.mkdirSync(binariesDir, { recursive: true });

  const binaryName = process.platform === 'win32' ? 'llama-helper.exe' : 'llama-helper';
  const sidecarName = process.platform === 'win32'
    ? `llama-helper-${targetTriple}.exe`
    : `llama-helper-${targetTriple}`;
  const sourceBinary = path.join(repoRoot, 'target', profile, binaryName);
  const destBinary = path.join(binariesDir, sidecarName);

  if (!fs.existsSync(sourceBinary)) {
    throw new Error(`Built llama-helper binary not found at ${sourceBinary}`);
  }

  fs.copyFileSync(sourceBinary, destBinary);
  if (process.platform !== 'win32') {
    fs.chmodSync(destBinary, 0o755);
  }

  console.log(`✅ Staged llama-helper sidecar at ${destBinary}`);
}

// Get the command (dev or build)
const command = process.argv[2];
if (!command || !['dev', 'build'].includes(command)) {
  console.error('Usage: node tauri-auto.js [dev|build]');
  process.exit(1);
}

// Detect GPU feature
let feature = '';

// Check for environment variable override first, including explicit CPU-only empty string.
const hasFeatureOverride = Object.prototype.hasOwnProperty.call(process.env, 'TAURI_GPU_FEATURE');
if (hasFeatureOverride) {
  feature = process.env.TAURI_GPU_FEATURE;
  if (feature) {
    console.log(`🔧 Using forced GPU feature from environment: ${feature}`);
  } else {
    console.log('🔧 Using forced CPU-only mode from environment');
  }
} else {
  try {
    const result = execSync('node scripts/auto-detect-gpu.js', {
      encoding: 'utf8',
      stdio: ['pipe', 'pipe', 'inherit']
    });
    feature = result.trim();
  } catch (err) {
    // If detection fails, continue with no features
  }
}

console.log(''); // Empty line for spacing

function getCudaArchitectures() {
  const existing = process.env.CMAKE_CUDA_ARCHITECTURES || process.env.CUDA_ARCHITECTURES;
  if (existing) {
    return existing;
  }

  try {
    const raw = execSync('nvidia-smi --query-gpu=compute_cap --format=csv,noheader', {
      encoding: 'utf8',
      stdio: ['pipe', 'pipe', 'pipe']
    }).trim();

    const caps = [...new Set(raw.split(/\r?\n/)
      .map(line => line.trim())
      .filter(Boolean)
      .map(line => {
        const match = line.match(/^(\d+)\.(\d+)$/);
        if (!match) {
          return null;
        }

        return `${match[1]}${match[2]}`;
      })
      .filter(Boolean))];

    if (caps.length > 0) {
      return caps.join(';');
    }
  } catch {
    // Fall through to the broad default list below.
  }

  return '61;70;75;80;86;89;90';
}

// Platform-specific environment variables
const platform = os.platform();
const env = { ...process.env };

if (platform === 'linux' && feature === 'cuda') {
  const cudaArchitectures = getCudaArchitectures();
  env.CMAKE_CUDA_ARCHITECTURES = cudaArchitectures;
  env.CMAKE_CUDA_STANDARD = env.CMAKE_CUDA_STANDARD || '17';
  env.CMAKE_POSITION_INDEPENDENT_CODE = env.CMAKE_POSITION_INDEPENDENT_CODE || 'ON';
  console.log(`🐧 Linux/CUDA detected: using CMAKE_CUDA_ARCHITECTURES=${cudaArchitectures}`);
}

// Build the tauri command
let tauriCmd = `tauri ${command}`;
if (feature && feature !== 'none') {
  tauriCmd += ` -- --features ${feature}`;
  console.log(`🚀 Running: tauri ${command} with features: ${feature}`);
} else {
  console.log(`🚀 Running: tauri ${command} (CPU-only mode)`);
}
console.log('');

// Execute the command
try {
  prepareLlamaHelperSidecar(command, env);
  console.log('');
  run(tauriCmd, { cwd: frontendDir, env });
} catch (err) {
  process.exit(err.status || 1);
}
