#!/usr/bin/env node

import { spawnSync } from 'node:child_process'
import { chmodSync, copyFileSync, mkdirSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url))
const PROJECT_ROOT = path.resolve(SCRIPT_DIR, '..')
const REPO_ROOT = process.env.DASCLAW_REPO_ROOT || path.resolve(PROJECT_ROOT, '..')
const PROFILE = process.env.DASCLAW_APP_SERVER_BUILD_PROFILE || 'release'
const BINARY_NAME = process.platform === 'win32' ? 'dasclaw-app-server.exe' : 'dasclaw-app-server'
const OUT_DIR = path.join(PROJECT_ROOT, '.bundle-resources', 'dasclaw-app-server')

if (PROFILE !== 'release' && PROFILE !== 'debug') {
  throw new Error('DASCLAW_APP_SERVER_BUILD_PROFILE must be release or debug')
}

const cargoArgs = ['build', '-p', 'dasclaw_app_server', '--bin', 'dasclaw-app-server']
if (PROFILE === 'release') {
  cargoArgs.push('--release')
}

console.log(`[build-dasclaw-app-server] cargo ${cargoArgs.join(' ')}`)
const cargo = spawnSync('cargo', cargoArgs, {
  cwd: REPO_ROOT,
  stdio: 'inherit',
  env: process.env
})

if (cargo.status !== 0) {
  process.exit(cargo.status ?? 1)
}

const targetRoot = process.env.CARGO_TARGET_DIR
  ? path.resolve(REPO_ROOT, process.env.CARGO_TARGET_DIR)
  : path.join(REPO_ROOT, 'target')
const sourceBinary = path.join(targetRoot, PROFILE, BINARY_NAME)
const targetBinary = path.join(OUT_DIR, BINARY_NAME)

mkdirSync(OUT_DIR, { recursive: true })
copyFileSync(sourceBinary, targetBinary)
if (process.platform !== 'win32') {
  chmodSync(targetBinary, 0o755)
}

console.log(`[build-dasclaw-app-server] bundled ${targetBinary}`)
