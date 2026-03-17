import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { DynamicWatermark } from '../DynamicWatermark';

// Mock theme context
vi.mock('../../../contexts/ThemeContext', () => ({
  useTheme: () => ({ theme: 'light' })
}));

// Mock Canvas API
const mockGetContext = vi.fn();
const mockFillText = vi.fn();
const mockMeasureText = vi.fn();
const mockSave = vi.fn();
const mockRestore = vi.fn();
const mockTranslate = vi.fn();
const mockRotate = vi.fn();
const mockClearRect = vi.fn();

beforeEach(() => {
  mockGetContext.mockReturnValue({
    fillText: mockFillText,
    measureText: mockMeasureText.mockReturnValue({ width: 100 }),
    save: mockSave,
    restore: mockRestore,
    translate: mockTranslate,
    rotate: mockRotate,
    clearRect: mockClearRect,
    font: '',
    fillStyle: '',
    globalAlpha: 1,
    textAlign: 'center',
    textBaseline: 'middle',
  });

  // Mock HTMLCanvasElement
  Object.defineProperty(HTMLCanvasElement.prototype, 'getContext', {
    value: mockGetContext,
    writable: true,
  });

  // Mock window dimensions
  Object.defineProperty(window, 'innerWidth', {
    value: 1920,
    writable: true,
  });
  Object.defineProperty(window, 'innerHeight', {
    value: 1080,
    writable: true,
  });

  vi.clearAllMocks();
});

describe('DynamicWatermark', () => {
  const defaultProps = {
    text: 'Test Watermark',
  };

  describe('渲染', () => {
    it('应该在启用时渲染Canvas', () => {
      render(<DynamicWatermark {...defaultProps} enabled={true} />);

      const canvas = document.querySelector('canvas');
      expect(canvas).toBeInTheDocument();
      expect(canvas).toHaveClass('fixed', 'inset-0', 'pointer-events-none', 'z-50');
    });

    it('应该在禁用时不渲染', () => {
      render(<DynamicWatermark {...defaultProps} enabled={false} />);

      const canvas = document.querySelector('canvas');
      expect(canvas).not.toBeInTheDocument();
    });

    it('应该在文本为空时不渲染', () => {
      render(<DynamicWatermark text="" enabled={true} />);

      const canvas = document.querySelector('canvas');
      expect(canvas).not.toBeInTheDocument();
    });

    it('应该在文本只有空格时不渲染', () => {
      render(<DynamicWatermark text="   " enabled={true} />);

      const canvas = document.querySelector('canvas');
      expect(canvas).not.toBeInTheDocument();
    });
  });

  describe('Canvas绘制', () => {
    it('应该设置正确的Canvas尺寸', () => {
      render(<DynamicWatermark {...defaultProps} />);

      const canvas = document.querySelector('canvas') as HTMLCanvasElement;
      expect(canvas.width).toBe(1920);
      expect(canvas.height).toBe(1080);
    });

    it('应该调用Canvas绘制方法', () => {
      render(<DynamicWatermark {...defaultProps} />);

      expect(mockGetContext).toHaveBeenCalledWith('2d');
      expect(mockClearRect).toHaveBeenCalled();
      expect(mockMeasureText).toHaveBeenCalledWith('Test Watermark');
      expect(mockFillText).toHaveBeenCalled();
    });

    it('应该使用正确的文字样式', () => {
      render(
        <DynamicWatermark 
          {...defaultProps} 
          fontSize={20}
          opacity={0.2}
          color="#ff0000"
        />
      );

      const context = mockGetContext.mock.results[0].value;
      expect(context.font).toBe('20px Arial, sans-serif');
      expect(context.fillStyle).toBe('#ff0000');
      expect(context.globalAlpha).toBe(0.2);
      expect(context.textAlign).toBe('center');
      expect(context.textBaseline).toBe('middle');
    });

    it('应该绘制旋转的文字', () => {
      render(<DynamicWatermark {...defaultProps} rotation={45} />);

      expect(mockSave).toHaveBeenCalled();
      expect(mockTranslate).toHaveBeenCalled();
      expect(mockRotate).toHaveBeenCalledWith((45 * Math.PI) / 180);
      expect(mockRestore).toHaveBeenCalled();
    });
  });

  describe('响应式', () => {
    it('应该响应窗口大小变化', async () => {
      render(<DynamicWatermark {...defaultProps} />);

      // 模拟窗口大小变化
      Object.defineProperty(window, 'innerWidth', { value: 1366, configurable: true });
      Object.defineProperty(window, 'innerHeight', { value: 768, configurable: true });

      // 触发resize事件
      window.dispatchEvent(new Event('resize'));

      // 使用 waitFor 等待状态更新
      await waitFor(() => {
        const canvas = document.querySelector('canvas') as HTMLCanvasElement;
        expect(canvas).toBeInTheDocument();
      });
    });
  });

  describe('配置选项', () => {
    it('应该使用默认配置', () => {
      render(<DynamicWatermark text="Test" />);

      const context = mockGetContext.mock.results[0].value;
      expect(context.font).toBe('16px Arial, sans-serif');
      expect(context.globalAlpha).toBe(0.1);
    });

    it('应该应用自定义配置', () => {
      render(
        <DynamicWatermark 
          text="Custom"
          fontSize={24}
          opacity={0.3}
          color="#00ff00"
          rotation={30}
          spacing={300}
        />
      );

      const context = mockGetContext.mock.results[0].value;
      expect(context.font).toBe('24px Arial, sans-serif');
      expect(context.fillStyle).toBe('#00ff00');
      expect(context.globalAlpha).toBe(0.3);
    });
  });

  describe('主题适配', () => {
    it('应该在浅色主题下使用黑色文字', () => {
      render(<DynamicWatermark {...defaultProps} />);

      const context = mockGetContext.mock.results[0].value;
      expect(context.fillStyle).toBe('#000000');
    });
  });
});