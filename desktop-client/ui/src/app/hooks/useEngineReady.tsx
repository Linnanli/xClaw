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
    const unlistenPromise = listen<{ type: string; data?: { type?: string; connected?: boolean } }>(
      'chat-stream',
      (event) => {
        // DataCustom connection_status: { type: "data-custom", data: { type: "connection_status", connected: true } }
        if (
          event.payload.type === 'data-custom' &&
          event.payload.data?.type === 'connection_status' &&
          event.payload.data?.connected
        ) {
          setReady(true);
          setReadyKey((k) => k + 1);
        }
      },
    );

    return () => { unlistenPromise.then((fn) => fn()).catch(() => {}); };
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
