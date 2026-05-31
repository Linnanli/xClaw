/**
 * E2E chat-stream 帧捕获 hook（仅 DEV + Tauri shell 下激活）。
 *
 * WebDriver 测试在 WKWebView 内执行 `window.__E2E_GET_EVENTS()` 回读真链路
 * 帧序，用于断言 `reasoning-start → reasoning-delta → reasoning-end` 等
 * 协议生命周期。production bundle 不会包含此模块（Vite tree-shake +
 * 入口处 `import.meta.env.DEV` gate）。
 */

import { listen } from '@tauri-apps/api/event';

interface CapturedEvent {
  timestamp: number;
  payload: unknown;
}

interface E2EWindow {
  __E2E_GET_EVENTS?: () => CapturedEvent[];
  __E2E_CLEAR_EVENTS?: () => void;
  __E2E_EVENT_COUNT?: () => number;
  __TAURI_INTERNALS__?: unknown;
}

export async function installE2EChatStreamCapture(): Promise<void> {
  const win = window as unknown as E2EWindow;
  if (!win.__TAURI_INTERNALS__) {
    return;
  }
  // 幂等：HMR / 多次 bootstrap 不应叠加监听器，否则同一事件会被 push 多次
  if (typeof win.__E2E_GET_EVENTS === 'function') {
    return;
  }

  const events: CapturedEvent[] = [];
  await listen<unknown>('chat-stream', (event) => {
    events.push({ timestamp: Date.now(), payload: event.payload });
  });

  win.__E2E_GET_EVENTS = () => events.slice();
  win.__E2E_CLEAR_EVENTS = () => {
    events.length = 0;
  };
  win.__E2E_EVENT_COUNT = () => events.length;
}
