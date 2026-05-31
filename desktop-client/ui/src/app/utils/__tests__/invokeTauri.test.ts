/**
 * invokeTauri 自动等待 + 重试单元测试
 *
 * 覆盖维度（AGENTS.md 测试维度矩阵）：
 * - 正常路径：成功不重试
 * - 契约：识别 JSON code=ENGINE_NOT_READY；识别旧版纯中文 fallback
 * - 失败路径：不可重试错误直接抛；超时仍 ready=false 时抛原错误
 * - 安全审计：防递归（get_engine_status 自身报 NOT_READY 不会触发等待）
 *
 * 单例 polling Promise 在每个 case 之间通过 __resetEngineReadyWaitForTesting 复位。
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// 必须在 import 待测模块前 mock @tauri-apps/api/core
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

import { invoke } from '@tauri-apps/api/core';
import {
  invokeTauri,
  isEngineNotReadyError,
  __resetEngineReadyWaitForTesting,
} from '../tauri';

const mockedInvoke = vi.mocked(invoke);

/**
 * 创建一个已预附 noop catch 的拒绝 Promise。
 * fake timers 下，深嵌套 await 链中 vitest 来不及同步附 handler，
 * Node 会触发 PromiseRejectionHandledWarning。预附静默 handler 不影响
 * 后续 `.then/await` 的语义（promise 仍可被 await 抛出），只防止误报。
 */
function rejected(err: unknown): Promise<never> {
  const p = Promise.reject(err);
  p.catch(() => {});
  return p;
}

beforeEach(() => {
  vi.useFakeTimers();
  __resetEngineReadyWaitForTesting();
  mockedInvoke.mockReset();
});

afterEach(() => {
  vi.useRealTimers();
});

describe('isEngineNotReadyError - 契约', () => {
  it('识别 JSON code=ENGINE_NOT_READY', () => {
    const err = JSON.stringify({
      code: 'ENGINE_NOT_READY',
      message: '引擎正在启动中，请稍后重试',
      retryable: true,
    });
    expect(isEngineNotReadyError(err)).toBe(true);
  });

  it('识别旧版纯中文 fallback', () => {
    expect(isEngineNotReadyError('引擎正在启动中，请稍后重试')).toBe(true);
  });

  it('结构化 retryable=false 拒绝重试', () => {
    const err = JSON.stringify({
      code: 'ENGINE_START_FAILED',
      message: '引擎启动失败: missing config',
      retryable: false,
    });
    expect(isEngineNotReadyError(err)).toBe(false);
  });

  it('无关错误返回 false', () => {
    expect(isEngineNotReadyError('SqlError: not found')).toBe(false);
    expect(isEngineNotReadyError(null)).toBe(false);
    expect(isEngineNotReadyError(undefined)).toBe(false);
    expect(isEngineNotReadyError({ message: 'something else' })).toBe(false);
  });

  it('支持 Error 对象的 message 字段', () => {
    const err = new Error(
      JSON.stringify({ code: 'ENGINE_NOT_READY', retryable: true, message: '...' }),
    );
    expect(isEngineNotReadyError(err)).toBe(true);
  });
});

describe('invokeTauri - 正常路径', () => {
  it('成功不重试，原样返回', async () => {
    mockedInvoke.mockResolvedValueOnce({ ok: true });
    const result = await invokeTauri('some_cmd', { x: 1 });
    expect(result).toEqual({ ok: true });
    expect(mockedInvoke).toHaveBeenCalledTimes(1);
    expect(mockedInvoke).toHaveBeenCalledWith('some_cmd', { x: 1 });
  });

  it('非 NOT_READY 错误直接抛，不轮询', async () => {
    mockedInvoke.mockRejectedValueOnce('real failure');
    await expect(invokeTauri('some_cmd')).rejects.toBe('real failure');
    expect(mockedInvoke).toHaveBeenCalledTimes(1);
  });
});

describe('invokeTauri - NOT_READY 自动等待 + 重试', () => {
  it('JSON code 触发等待 → ready → 重试成功（只重试一次）', async () => {
    const notReadyErr = JSON.stringify({
      code: 'ENGINE_NOT_READY',
      retryable: true,
      message: '...',
    });
    mockedInvoke
      .mockRejectedValueOnce(notReadyErr) // 原 IPC 第一次
      .mockResolvedValueOnce({ ready: true }) // get_engine_status 第一次
      .mockResolvedValueOnce({ data: 'ok' }); // 原 IPC 重试

    const promise = invokeTauri('list_threads');
    await vi.advanceTimersByTimeAsync(50);
    const result = await promise;

    expect(result).toEqual({ data: 'ok' });
    expect(mockedInvoke).toHaveBeenCalledTimes(3);
    expect(mockedInvoke.mock.calls[0]?.[0]).toBe('list_threads');
    expect(mockedInvoke.mock.calls[1]?.[0]).toBe('get_engine_status');
    expect(mockedInvoke.mock.calls[2]?.[0]).toBe('list_threads');
  });

  it('字符串 fallback 也触发等待 + 重试', async () => {
    mockedInvoke
      .mockRejectedValueOnce('引擎正在启动中，请稍后重试')
      .mockResolvedValueOnce({ ready: true })
      .mockResolvedValueOnce('ok');

    const promise = invokeTauri('some_cmd');
    await vi.advanceTimersByTimeAsync(50);
    await expect(promise).resolves.toBe('ok');
    expect(mockedInvoke).toHaveBeenCalledTimes(3);
  });

  it('重试时再报 NOT_READY 不再二次等待，直接抛原错误', async () => {
    const err = JSON.stringify({ code: 'ENGINE_NOT_READY', retryable: true, message: '...' });
    mockedInvoke
      .mockImplementationOnce(() => rejected(err)) // 原 IPC
      .mockResolvedValueOnce({ ready: true }) // poll
      .mockImplementationOnce(() => rejected(err)); // 重试又 NOT_READY

    const promise = invokeTauri('some_cmd');
    promise.catch(() => {}); // 预附 guard，避免 advance timers 期间被计入 unhandled
    await vi.advanceTimersByTimeAsync(50);
    await expect(promise).rejects.toBe(err);
    expect(mockedInvoke).toHaveBeenCalledTimes(3);
  });
});

describe('invokeTauri - 失败路径', () => {
  it('轮询超时（始终 not ready）抛出原 NOT_READY 错误', async () => {
    const err = JSON.stringify({ code: 'ENGINE_NOT_READY', retryable: true, message: '...' });
    mockedInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_engine_status') return Promise.resolve({ ready: false });
      return rejected(err);
    });

    const promise = invokeTauri('some_cmd');
    promise.catch(() => {}); // 预附 guard
    // 推进 16s 越过 15s 上限
    await vi.advanceTimersByTimeAsync(16_000);
    await expect(promise).rejects.toBe(err);
  });
});

describe('invokeTauri - 安全审计 / 防递归', () => {
  it('get_engine_status 自身报 NOT_READY 不会进入等待循环', async () => {
    const err = JSON.stringify({ code: 'ENGINE_NOT_READY', retryable: true, message: '...' });
    mockedInvoke.mockRejectedValueOnce(err);

    await expect(invokeTauri('get_engine_status')).rejects.toBe(err);
    // 只调用 1 次（原始那次），没有再 poll
    expect(mockedInvoke).toHaveBeenCalledTimes(1);
  });
});
