#!/usr/bin/env node

import { spawn } from 'node:child_process';
import { createRequire } from 'node:module';
import { mkdtempSync, rmSync } from 'node:fs';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const WebSocket = require('ws');

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const PROJECT_ROOT = path.resolve(SCRIPT_DIR, '..');
const REPO_ROOT = process.env.DASCLAW_REPO_ROOT || path.resolve(PROJECT_ROOT, '..');
const HOST = '127.0.0.1';
const PREVIEW_PORT = Number(process.env.DASCLAW_SMOKE_PREVIEW_PORT || 4173);
const DEBUG_PORT = Number(process.env.DASCLAW_SMOKE_DEBUG_PORT || 9223);
const PROMPT = process.env.DASCLAW_SMOKE_PROMPT || 'hello smoke';
const EXPECTED_DELTA = `echo: ${PROMPT}`;
const SKIP_BUILD = process.env.DASCLAW_SMOKE_SKIP_BUILD === '1';
const VERBOSE = process.env.DASCLAW_SMOKE_VERBOSE === '1';
const CDP_REQUEST_TIMEOUT_MS = 40_000;
const VITE_PACKAGE_PATH = require.resolve('vite/package.json');
const VITE_BIN = path.join(path.dirname(VITE_PACKAGE_PATH), require(VITE_PACKAGE_PATH).bin.vite);
const ELECTRON_PATH = require('electron');

const children = new Set();
let userDataDir;
let workspaceDir;
let cleanedUp = false;

main().catch(async (error) => {
  console.error(`[dasclaw-smoke] FAIL: ${error.stack || error.message}`);
  await cleanup();
  process.exit(1);
});

async function main() {
  console.log('[dasclaw-smoke] Starting Electron app-server smoke');
  await ensureDevToolsPortFree();

  if (!SKIP_BUILD) {
    await runChecked('vite build', process.execPath, [VITE_BIN, 'build'], {
      cwd: PROJECT_ROOT,
      stdio: 'inherit',
    });
  }

  userDataDir = mkdtempSync(path.join(os.tmpdir(), 'open-cowork-dasclaw-smoke-user-data-'));
  workspaceDir = mkdtempSync(path.join(os.tmpdir(), 'open-cowork-dasclaw-smoke-work-'));

  const preview = spawnManaged(
    process.execPath,
    [VITE_BIN, 'preview', '--host', HOST, '--port', String(PREVIEW_PORT), '--strictPort'],
    {
      cwd: PROJECT_ROOT,
      label: 'vite-preview',
    }
  );
  await waitForHttp(`http://${HOST}:${PREVIEW_PORT}/`, 30_000);

  const electron = spawnManaged(
    ELECTRON_PATH,
    ['.', '--no-sandbox', `--user-data-dir=${userDataDir}`],
    {
      cwd: PROJECT_ROOT,
      env: buildElectronEnv(),
      label: 'electron',
    }
  );

  const pageTarget = await waitForRendererTarget(45_000);
  const result = await runRendererSmoke(pageTarget.webSocketDebuggerUrl);
  if (VERBOSE) {
    console.log(`[dasclaw-smoke] Renderer result: ${JSON.stringify(result, null, 2)}`);
  }

  if (!result.electronAPI) {
    throw new Error('renderer did not expose window.electronAPI');
  }
  if (!result.afterConfigured) {
    throw new Error('temporary config did not satisfy app credential guard');
  }
  if (!result.threadId) {
    throw new Error('session did not bind to a dasclaw thread id');
  }
  if (!result.hasEchoDelta) {
    throw new Error(
      `renderer did not receive expected delta: ${EXPECTED_DELTA}; events=${JSON.stringify(
        result.statusEvents
      )}`
    );
  }
  if (!result.hasIdle) {
    throw new Error('session did not reach idle terminal status');
  }
  if (result.partialCount !== 1) {
    throw new Error(`expected exactly one stream.partial, got ${result.partialCount}`);
  }

  console.log('[dasclaw-smoke] PASS');
  console.log(`[dasclaw-smoke] session=${result.sessionId} thread=${result.threadId}`);
  console.log(`[dasclaw-smoke] partial=${JSON.stringify(result.partials[0]?.payload || {})}`);

  electron.kill('SIGTERM');
  preview.kill('SIGTERM');
  await cleanup();
}

function buildElectronEnv() {
  const env = {
    ...process.env,
    OPEN_COWORK_AGENT_RUNNER: 'dasclaw',
    DASCLAW_APP_SERVER_RUNTIME: 'echo',
    DASCLAW_REPO_ROOT: REPO_ROOT,
    VITE_DEV_SERVER_URL: `http://${HOST}:${PREVIEW_PORT}/`,
  };

  for (const key of [
    'ANTHROPIC_API_KEY',
    'ANTHROPIC_AUTH_TOKEN',
    'ANTHROPIC_BASE_URL',
    'OPENAI_API_KEY',
    'OPENAI_BASE_URL',
    'OPENAI_MODEL',
    'GEMINI_API_KEY',
    'GEMINI_BASE_URL',
  ]) {
    delete env[key];
  }

  return env;
}

function spawnManaged(command, args, options = {}) {
  const { label = path.basename(command), env = process.env, cwd = PROJECT_ROOT, stdio } = options;
  const child = spawn(command, args, {
    cwd,
    env,
    stdio: stdio || ['ignore', 'pipe', 'pipe'],
  });
  children.add(child);
  child.once('exit', () => children.delete(child));
  child.on('error', (error) => {
    console.error(`[dasclaw-smoke] ${label} failed to start: ${error.message}`);
  });
  if (!stdio || stdio === 'pipe') {
    pipeWithPrefix(child.stdout, label);
    pipeWithPrefix(child.stderr, label);
  }
  return child;
}

function pipeWithPrefix(stream, label) {
  if (!stream) return;
  stream.setEncoding('utf8');
  stream.on('data', (chunk) => {
    for (const line of String(chunk).split(/\r?\n/)) {
      if (line.trim()) {
        console.log(`[${label}] ${line}`);
      }
    }
  });
}

async function runChecked(label, command, args, options) {
  await new Promise((resolve, reject) => {
    const child = spawnManaged(command, args, { ...options, label });
    child.once('exit', (code, signal) => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`${label} exited with ${signal || `code ${code}`}`));
      }
    });
  });
}

function getJson(url) {
  return new Promise((resolve, reject) => {
    const request = http.get(url, (response) => {
      let body = '';
      response.setEncoding('utf8');
      response.on('data', (chunk) => {
        body += chunk;
      });
      response.on('end', () => {
        if (response.statusCode && response.statusCode >= 400) {
          reject(new Error(`${url} returned HTTP ${response.statusCode}`));
          return;
        }
        try {
          resolve(JSON.parse(body));
        } catch (error) {
          reject(error);
        }
      });
    });
    request.on('error', reject);
    request.setTimeout(2_000, () => {
      request.destroy(new Error(`${url} timed out`));
    });
  });
}

async function waitForHttp(url, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      await new Promise((resolve, reject) => {
        const request = http.get(url, (response) => {
          response.resume();
          if (response.statusCode && response.statusCode < 500) {
            resolve();
          } else {
            reject(new Error(`HTTP ${response.statusCode}`));
          }
        });
        request.on('error', reject);
        request.setTimeout(1_000, () => request.destroy(new Error('timeout')));
      });
      return;
    } catch {
      await delay(250);
    }
  }
  throw new Error(`timed out waiting for ${url}`);
}

async function ensureDevToolsPortFree() {
  try {
    const targets = await getJson(`http://${HOST}:${DEBUG_PORT}/json/list`);
    if (Array.isArray(targets) && targets.length > 0) {
      throw new Error(
        `DevTools port ${DEBUG_PORT} is already in use; close the existing Electron smoke app first`
      );
    }
  } catch (error) {
    if (String(error.message || '').includes('already in use')) {
      throw error;
    }
  }
}

async function waitForRendererTarget(timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  const previewUrl = `http://${HOST}:${PREVIEW_PORT}/`;
  while (Date.now() < deadline) {
    try {
      const targets = await getJson(`http://${HOST}:${DEBUG_PORT}/json/list`);
      const target = targets.find(
        (item) =>
          item.type === 'page' &&
          typeof item.url === 'string' &&
          item.url.startsWith(previewUrl) &&
          item.webSocketDebuggerUrl
      );
      if (target) return target;
    } catch {
      // Electron may not have opened the DevTools endpoint yet.
    }
    await delay(250);
  }
  throw new Error('timed out waiting for Electron renderer DevTools target');
}

async function runRendererSmoke(webSocketDebuggerUrl) {
  const ws = new WebSocket(webSocketDebuggerUrl);
  let nextId = 1;
  const pending = new Map();

  const rejectPending = (error) => {
    for (const [id, callbacks] of pending.entries()) {
      clearTimeout(callbacks.timer);
      pending.delete(id);
      callbacks.reject(error);
    }
  };

  ws.on('message', (data) => {
    const message = JSON.parse(String(data));
    if (!message.id || !pending.has(message.id)) return;
    const callbacks = pending.get(message.id);
    pending.delete(message.id);
    clearTimeout(callbacks.timer);
    if (message.error) {
      callbacks.reject(new Error(message.error.message));
    } else {
      callbacks.resolve(message.result);
    }
  });
  ws.on('error', (error) => {
    rejectPending(error);
  });
  ws.on('close', () => {
    rejectPending(new Error('CDP WebSocket closed before smoke completed'));
  });

  await new Promise((resolve, reject) => {
    ws.once('open', resolve);
    ws.once('error', reject);
  });

  const send = (method, params = {}) => {
    const id = nextId++;
    ws.send(JSON.stringify({ id, method, params }));
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        pending.delete(id);
        reject(new Error(`CDP request timed out: ${method}`));
      }, CDP_REQUEST_TIMEOUT_MS);
      pending.set(id, { resolve, reject, timer });
    });
  };

  await send('Runtime.enable');
  const result = await send('Runtime.evaluate', {
    expression: rendererSmokeExpression(),
    awaitPromise: true,
    returnByValue: true,
    timeout: 35_000,
  });
  ws.close();

  if (result.exceptionDetails) {
    throw new Error(JSON.stringify(result.exceptionDetails, null, 2));
  }

  return result.result.value;
}

function rendererSmokeExpression() {
  return `
(async () => {
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const readyDeadline = Date.now() + 10000;
  while (Date.now() < readyDeadline && typeof window.__getNavStatus !== 'function') {
    await sleep(100);
  }
  if (typeof window.__getNavStatus !== 'function') {
    throw new Error('renderer navigation helpers were not ready');
  }
  await sleep(500);
  const events = [];
  const off = window.electronAPI.on((event) => events.push(event));
  await sleep(500);
  const before = await window.electronAPI.config.get();
  await window.electronAPI.config.save({
    provider: 'openai',
    activeProfileKey: 'openai',
    apiKey: 'sk-dasclaw-smoke',
    baseUrl: 'https://api.openai.com/v1',
    model: 'gpt-5.4',
    sandboxEnabled: false,
    memoryEnabled: false
  });
  await sleep(500);
  const after = await window.electronAPI.config.get();
  const session = await window.electronAPI.invoke({
    type: 'session.start',
    payload: {
      title: 'Dasclaw Smoke',
      prompt: ${JSON.stringify(PROMPT)},
      cwd: ${JSON.stringify(workspaceDir)},
      allowedTools: [],
      content: [{ type: 'text', text: ${JSON.stringify(PROMPT)} }],
      memoryEnabled: false
    }
  });

  const deadline = Date.now() + 30000;
  while (Date.now() < deadline) {
    const partials = events.filter((event) => event.type === 'stream.partial' && event.payload?.sessionId === session.id);
    const hasEchoDelta = partials.some((event) => String(event.payload?.delta || '').includes(${JSON.stringify(EXPECTED_DELTA)}));
    const hasIdle = events.some((event) => event.type === 'session.status' && event.payload?.sessionId === session.id && event.payload?.status === 'idle');
    if (hasEchoDelta && hasIdle) break;
    await sleep(250);
  }

  await window.electronAPI.invoke({ type: 'session.stop', payload: { sessionId: session.id } });
  await sleep(250);
  off?.();

  const partials = events.filter((event) => event.type === 'stream.partial' && event.payload?.sessionId === session.id);
  const statusEvents = events.filter((event) => event.type === 'session.status' && event.payload?.sessionId === session.id);
  return {
    electronAPI: !!window.electronAPI,
    beforeConfigured: !!before?.isConfigured,
    afterConfigured: !!after?.isConfigured,
    sessionId: session.id,
    threadId: session.claudeSessionId,
    partialCount: partials.length,
    partials,
    statusEvents,
    hasEchoDelta: partials.some((event) => String(event.payload?.delta || '').includes(${JSON.stringify(EXPECTED_DELTA)})),
    hasIdle: statusEvents.some((event) => event.payload?.status === 'idle')
  };
})()
`;
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function cleanup() {
  if (cleanedUp) return;
  cleanedUp = true;

  for (const child of Array.from(children)) {
    if (isRunning(child)) {
      child.kill('SIGTERM');
    }
  }

  await delay(500);

  for (const child of Array.from(children)) {
    if (isRunning(child)) {
      child.kill('SIGKILL');
    }
  }

  if (userDataDir) {
    rmSync(userDataDir, { recursive: true, force: true });
  }
  if (workspaceDir) {
    rmSync(workspaceDir, { recursive: true, force: true });
  }
}

for (const signal of ['SIGINT', 'SIGTERM']) {
  process.once(signal, async () => {
    await cleanup();
    process.kill(process.pid, signal);
  });
}

process.once('exit', () => {
  if (!cleanedUp) {
    for (const child of Array.from(children)) {
      if (isRunning(child)) child.kill('SIGTERM');
    }
  }
});

function isRunning(child) {
  return child.exitCode === null && child.signalCode === null;
}
