import { expect, afterEach, vi } from 'vitest';
import { cleanup, waitFor } from '@testing-library/react';
import * as matchers from '@testing-library/jest-dom/matchers';
import userEvent from '@testing-library/user-event';

// Extend Vitest's expect with jest-dom matchers
expect.extend(matchers);

// Cleanup after each test
afterEach(() => {
  cleanup();
});

/**
 * 点击 antd Popconfirm 的确认按钮。
 *
 * 直接用 document.querySelector 拿到按钮后立即点击是不可靠的——
 * Popconfirm 的 portal 在 jsdom 里渲染有延迟。
 * 此函数用 waitFor 等待按钮出现后再点击，并在按钮不存在时让测试明确失败。
 */
export async function clickPopconfirmOk(): Promise<void> {
  const user = userEvent.setup();
  const btn = await waitFor(() => {
    const el = document.querySelector(
      '.ant-popconfirm-buttons .ant-btn-primary'
    ) as HTMLElement | null;
    expect(el).not.toBeNull();
    return el!;
  });
  await user.click(btn);
}

// Mock window.matchMedia
Object.defineProperty(window, 'matchMedia', {
  writable: true,
  value: vi.fn().mockImplementation((query) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  })),
});

// Mock localStorage
const localStorageMock = {
  getItem: vi.fn(),
  setItem: vi.fn(),
  removeItem: vi.fn(),
  clear: vi.fn(),
};
global.localStorage = localStorageMock as any;

// Mock ResizeObserver
class ResizeObserverMock {
  observe() {}
  unobserve() {}
  disconnect() {}
}

global.ResizeObserver = ResizeObserverMock as any;

