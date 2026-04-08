/**
 * useRunningJobs — 追踪运行中任务数
 *
 * 两种触发机制：
 * 1. 定期轮询（10s）— 保底兜底
 * 2. 收到 chat-event response 时立即刷新 — Agent 工具调用结束后快速响应
 *
 * 引擎就绪时触发首次查询。
 */

import { useState, useEffect, useCallback } from 'react';
import { listen } from '@tauri-apps/api/event';
import { jobApi } from '../utils/tauri';
import { useEngineReady } from './useEngineReady';

const POLL_INTERVAL_MS = 10_000;

export function useRunningJobs(): number {
  const [count, setCount] = useState(0);
  const { readyKey } = useEngineReady();

  const refresh = useCallback(async () => {
    try {
      const jobs = await jobApi.getJobs();
      const running = jobs.filter(
        (j) => j.status === 'pending' || j.status === 'in_progress',
      ).length;
      setCount(running);
    } catch {
      // 引擎未就绪时静默失败，保持上次计数
    }
  }, []);

  // 引擎就绪时立即刷新
  useEffect(() => {
    refresh();
  }, [readyKey, refresh]);

  // 定期轮询（兜底）
  useEffect(() => {
    const id = setInterval(refresh, POLL_INTERVAL_MS);
    return () => clearInterval(id);
  }, [refresh]);

  // job_status 事件触发立即刷新（任务创建/完成时精确响应）
  useEffect(() => {
    const unlistenPromise = listen<{ type: string }>('chat-event', (event) => {
      if (event.payload.type === 'job_status') {
        refresh();
      }
    });
    return () => { unlistenPromise.then((fn) => fn()).catch(() => {}); };
  }, [refresh]);

  return count;
}
