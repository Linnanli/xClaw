/**
 * useEngineReady — 引擎就绪状态管理
 *
 * 集中监听 Rust 引擎的 `connection_status` 事件，
 * 暴露 `readyKey` 计数器供各组件在 useEffect 依赖中使用。
 * 引擎就绪时 readyKey 自增，触发所有依赖组件重新加载数据，
 * 解决"前端先于引擎加载导致数据为空"的问题。
 */

import { createContext, useContext, useState, useEffect, type ReactNode } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';

interface EngineReadyState {
  /** 引擎是否已就绪 */
  ready: boolean;
  /** 每次引擎就绪时自增，用作 useEffect 依赖触发数据重载 */
  readyKey: number;
}

const EngineReadyContext = createContext<EngineReadyState>({ ready: false, readyKey: 0 });

export function EngineReadyProvider({ children }: { children: ReactNode }) {
  const [ready, setReady] = useState(false);
  const [readyKey, setReadyKey] = useState(0);

  useEffect(() => {
    let cancelled = false;

    const markReady = () => {
      if (cancelled) return;
      setReady(true);
      setReadyKey((k) => k + 1);
    };

    // 1) 挂载时主动查询当前引擎状态作为初始值。
    //    引擎就绪事件是一次性广播，webview 刷新后会错过，
    //    必须在挂载时回查可查询状态，否则会永久卡在"引擎启动中…"。
    invoke<{ ready: boolean }>('get_engine_status')
      .then((status) => {
        if (status?.ready) markReady();
      })
      .catch(() => {});

    // 2) 订阅后续就绪事件（覆盖"挂载时引擎尚未就绪"的情况）。
    const unlistenPromise = listen<{ type: string; data?: { type?: string; connected?: boolean } }>(
      'chat-stream',
      (event) => {
        // DataCustom connection_status: { type: "data-custom", data: { type: "connection_status", connected: true } }
        if (
          event.payload.type === 'data-custom' &&
          event.payload.data?.type === 'connection_status' &&
          event.payload.data?.connected
        ) {
          markReady();
        }
      },
    );

    return () => {
      cancelled = true;
      unlistenPromise.then((fn) => fn()).catch(() => {});
    };
  }, []);

  return (
    <EngineReadyContext.Provider value={{ ready, readyKey }}>
      {children}
    </EngineReadyContext.Provider>
  );
}

export function useEngineReady(): EngineReadyState {
  return useContext(EngineReadyContext);
}
