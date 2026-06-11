import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';

const electronApp = vi.hoisted(() => ({
  isPackaged: false,
}));
const spawnMock = vi.hoisted(() => vi.fn(() => ({ pid: 1234 })));

vi.mock('electron', () => ({
  app: electronApp,
}));

vi.mock('child_process', () => ({
  spawn: spawnMock,
}));

import {
  resolveBundledAppServerBinary,
  spawnDefaultAppServer,
} from '../src/main/dasclaw/app-server-rpc';

const tempDirs: string[] = [];
const originalResourcesPath = process.resourcesPath;
const originalExplicitBinary = process.env.DASCLAW_APP_SERVER_BIN;

afterEach(() => {
  electronApp.isPackaged = false;
  spawnMock.mockClear();
  if (originalResourcesPath === undefined) {
    delete process.resourcesPath;
  } else {
    Object.defineProperty(process, 'resourcesPath', {
      configurable: true,
      value: originalResourcesPath,
    });
  }
  if (originalExplicitBinary === undefined) {
    delete process.env.DASCLAW_APP_SERVER_BIN;
  } else {
    process.env.DASCLAW_APP_SERVER_BIN = originalExplicitBinary;
  }
  for (const dir of tempDirs.splice(0)) {
    rmSync(dir, { recursive: true, force: true });
  }
});

function tempResourcesDir(): string {
  const dir = mkdtempSync(join(tmpdir(), 'dasclaw-app-server-rpc-'));
  tempDirs.push(dir);
  return dir;
}

function createBundledBinary(resourcesPath: string, platform: NodeJS.Platform): string {
  const bundleDir = join(resourcesPath, 'dasclaw-app-server');
  const binaryName = platform === 'win32' ? 'dasclaw-app-server.exe' : 'dasclaw-app-server';
  mkdirSync(bundleDir, { recursive: true });
  const binary = join(bundleDir, binaryName);
  writeFileSync(binary, 'binary');
  return binary;
}

describe('resolveBundledAppServerBinary', () => {
  it('discovers the packaged dasclaw app-server resource for the host platform', () => {
    const resourcesPath = tempResourcesDir();
    const binary = createBundledBinary(resourcesPath, 'darwin');

    expect(resolveBundledAppServerBinary(resourcesPath, 'darwin')).toBe(binary);
  });

  it('uses the Windows executable name when resolving packaged resources', () => {
    const resourcesPath = tempResourcesDir();
    const binary = createBundledBinary(resourcesPath, 'win32');

    expect(resolveBundledAppServerBinary(resourcesPath, 'win32')).toBe(binary);
  });

  it('does not fall back to cargo paths in packaged resource resolution', () => {
    const resourcesPath = tempResourcesDir();

    expect(resolveBundledAppServerBinary(resourcesPath, 'linux')).toBeNull();
  });
});

describe('spawnDefaultAppServer', () => {
  it('spawns the bundled binary in packaged mode without requiring DASCLAW_APP_SERVER_BIN', () => {
    const resourcesPath = tempResourcesDir();
    const binary = createBundledBinary(resourcesPath, process.platform);
    delete process.env.DASCLAW_APP_SERVER_BIN;
    electronApp.isPackaged = true;
    Object.defineProperty(process, 'resourcesPath', {
      configurable: true,
      value: resourcesPath,
    });

    const child = spawnDefaultAppServer();

    expect(child).toEqual({ pid: 1234 });
    expect(spawnMock).toHaveBeenCalledWith(binary, [], {
      stdio: 'pipe',
      env: expect.objectContaining({}),
    });
  });

  it('fails explicitly in packaged mode when the bundled binary is missing', () => {
    const resourcesPath = tempResourcesDir();
    delete process.env.DASCLAW_APP_SERVER_BIN;
    electronApp.isPackaged = true;
    Object.defineProperty(process, 'resourcesPath', {
      configurable: true,
      value: resourcesPath,
    });

    expect(() => spawnDefaultAppServer()).toThrow(
      'Packaged dasclaw app-server binary was not found'
    );
    expect(spawnMock).not.toHaveBeenCalled();
  });
});
