import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const FALLBACK_USER_DATA_DIR = path.join(process.cwd(), '.cowork-user-data');
const FALLBACK_LOGS_DIR = path.join(FALLBACK_USER_DATA_DIR, 'logs');

let capturedWriteStream: fs.WriteStream | null = null;

async function closeAndReadLog(
  logger: typeof import('../src/main/utils/logger'),
  logFilePath: string
): Promise<string> {
  const stream = capturedWriteStream;

  const waitForFlush =
    stream && !stream.writableFinished
      ? new Promise<void>((resolve) => {
          stream.once('finish', resolve);
          stream.once('close', resolve);
        })
      : new Promise<void>((resolve) => {
          setTimeout(resolve, 50);
        });

  logger.closeLogFile();
  await waitForFlush;

  if (!fs.existsSync(logFilePath)) {
    await new Promise<void>((resolve) => {
      setTimeout(resolve, 50);
    });
  }

  if (!fs.existsSync(logFilePath) && stream && !stream.destroyed) {
    await new Promise<void>((resolve) => {
      stream.once('finish', resolve);
      stream.once('close', resolve);
    });
  }

  return fs.readFileSync(logFilePath, 'utf8');
}

describe('logger fallback behavior', () => {
  beforeEach(() => {
    vi.resetModules();
    vi.restoreAllMocks();
    capturedWriteStream = null;

    const realCreateWriteStream = fs.createWriteStream.bind(fs);
    vi.spyOn(fs, 'createWriteStream').mockImplementation(
      (...args: Parameters<typeof fs.createWriteStream>) => {
        const stream = realCreateWriteStream(...args);
        capturedWriteStream = stream;
        return stream;
      }
    );
  });

  afterEach(() => {
    vi.restoreAllMocks();
    capturedWriteStream = null;
  });

  it('uses fallback userData path when electron app path API is unavailable', async () => {
    vi.doMock('electron', () => ({
      app: {},
    }));

    const logger = await import('../src/main/utils/logger');

    expect(logger.getLogsDirectory()).toBe(FALLBACK_LOGS_DIR);

    // Trigger lazy init by writing a log entry
    logger.log('init');
    const logFilePath = logger.getLogFilePath();
    expect(logFilePath).toBeTruthy();
    expect(logFilePath?.startsWith(FALLBACK_LOGS_DIR)).toBe(true);
    logger.closeLogFile();
  });

  it('recovers when active log file is removed unexpectedly', async () => {
    vi.doMock('electron', () => ({
      app: {},
    }));

    const logger = await import('../src/main/utils/logger');
    // Trigger lazy init by writing a log entry
    logger.log('init');
    const initialLogPath = logger.getLogFilePath();
    expect(initialLogPath).toBeTruthy();

    if (initialLogPath && fs.existsSync(initialLogPath)) {
      fs.unlinkSync(initialLogPath);
    }

    const randomSpy = vi.spyOn(Math, 'random').mockReturnValue(0);
    expect(() => logger.log('trigger rotate check')).not.toThrow();
    randomSpy.mockRestore();

    const recoveredLogPath = logger.getLogFilePath();
    expect(recoveredLogPath).toBeTruthy();
    expect(recoveredLogPath?.startsWith(FALLBACK_LOGS_DIR)).toBe(true);
    logger.closeLogFile();
  });

  it('serializes Error objects with message details instead of empty object', async () => {
    vi.doMock('electron', () => ({
      app: {},
    }));

    const logger = await import('../src/main/utils/logger');
    // Trigger lazy init by writing a log entry
    logger.log('init');
    const logFilePath = logger.getLogFilePath();
    expect(logFilePath).toBeTruthy();

    logger.logError('[test] expected-error', new Error('boom logger error'));
    const content = await closeAndReadLog(logger, logFilePath!);
    expect(content).toContain('[test] expected-error');
    expect(content).toContain('boom logger error');
  });

  it('reopens a fresh log file after closeLogFile is called', async () => {
    vi.doMock('electron', () => ({
      app: {},
    }));

    const logger = await import('../src/main/utils/logger');
    // Trigger lazy init by writing a log entry
    logger.log('init');
    const firstLogPath = logger.getLogFilePath();
    expect(firstLogPath).toBeTruthy();

    logger.closeLogFile();
    logger.log('after close should reopen');

    const secondLogPath = logger.getLogFilePath();
    expect(secondLogPath).toBeTruthy();
    expect(secondLogPath).not.toBe(firstLogPath);
  });

  it('does not delete recently-created log files during cleanup', async () => {
    const testUserDataDir = fs.mkdtempSync(path.join(os.tmpdir(), 'cowork-logger-cleanup-'));
    vi.doMock('electron', () => ({
      app: {
        getPath: (_name: string) => testUserDataDir,
        getVersion: () => 'test',
      },
    }));

    const logsDir = path.join(testUserDataDir, 'logs');
    fs.mkdirSync(logsDir, { recursive: true });
    const recentLogPath = path.join(logsDir, 'app-2099-01-01_00-00-00-000-recent.log');

    fs.writeFileSync(recentLogPath, 'recent active peer log');
    for (let index = 0; index < 8; index += 1) {
      const oldLogPath = path.join(logsDir, `app-2000-01-01_00-00-00-00${index}.log`);
      fs.writeFileSync(oldLogPath, 'old log');
      const oldDate = new Date(Date.now() - 120_000 - index * 1000);
      fs.utimesSync(oldLogPath, oldDate, oldDate);
    }

    const logger = await import('../src/main/utils/logger');
    logger.log('trigger cleanup');
    const logFilePath = logger.getLogFilePath();
    expect(logFilePath).toBeTruthy();
    await closeAndReadLog(logger, logFilePath!);

    expect(fs.existsSync(recentLogPath)).toBe(true);

    fs.rmSync(testUserDataDir, { recursive: true, force: true });
  });

  it('swallows broken pipe errors from console output', async () => {
    vi.doMock('electron', () => ({
      app: {},
    }));

    const logger = await import('../src/main/utils/logger');
    const epipeError = Object.assign(new Error('write EPIPE'), { code: 'EPIPE' });
    const consoleSpy = vi.spyOn(console, 'log').mockImplementation(() => {
      throw epipeError;
    });

    expect(() => logger.log('stdout closed')).not.toThrow();

    consoleSpy.mockRestore();
    logger.closeLogFile();
  });
});
