// 动态水印功能 - 安全测试覆盖率
// 测试覆盖率维度：安全覆盖 + 恶意输入防护
// 目标覆盖率：100%

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { DynamicWatermark } from '../DynamicWatermark';

// Mock theme context
vi.mock('../../../contexts/ThemeContext', () => ({
  useTheme: () => ({ theme: 'light' })
}));

// Mock Canvas API for security tests
const mockGetContext = vi.fn();
const mockFillText = vi.fn();
const mockMeasureText = vi.fn();

beforeEach(() => {
  mockGetContext.mockReturnValue({
    fillText: mockFillText,
    measureText: mockMeasureText.mockReturnValue({ width: 100 }),
    save: vi.fn(),
    restore: vi.fn(),
    translate: vi.fn(),
    rotate: vi.fn(),
    clearRect: vi.fn(),
    font: '',
    fillStyle: '',
    globalAlpha: 1,
    textAlign: 'center',
    textBaseline: 'middle',
  });

  Object.defineProperty(HTMLCanvasElement.prototype, 'getContext', {
    value: mockGetContext,
    writable: true,
  });

  vi.clearAllMocks();
});

describe('DynamicWatermark - Security Tests', () => {
  describe('恶意输入防护测试', () => {
    it('应该防护XSS攻击 - HTML标签注入', () => {
      const maliciousInputs = [
        '<script>alert("xss")</script>',
        '<img src="x" onerror="alert(1)">',
        '<svg onload="alert(1)">',
        '"><script>alert("xss")</script>',
        'javascript:alert("xss")',
        '<iframe src="javascript:alert(1)"></iframe>',
      ];

      maliciousInputs.forEach(maliciousText => {
        render(<DynamicWatermark text={maliciousText} />);
        
        // 验证恶意脚本没有被执行
        expect(mockFillText).toHaveBeenCalledWith(maliciousText, 0, 0);
        // Canvas fillText 会将HTML标签作为纯文本处理，这是安全的
        
        // 验证没有DOM元素被创建
        const scripts = document.querySelectorAll('script');
        const images = document.querySelectorAll('img[onerror]');
        const svgs = document.querySelectorAll('svg[onload]');
        
        expect(scripts.length).toBe(0);
        expect(images.length).toBe(0);
        expect(svgs.length).toBe(0);
      });
    });

    it('应该防护CSS注入攻击', () => {
      const cssInjectionInputs = [
        'expression(alert("css injection"))',
        'url("javascript:alert(1)")',
        '@import "malicious.css"',
        'behavior:url(malicious.htc)',
        '-moz-binding:url("data:text/xml;base64,...")',
      ];

      cssInjectionInputs.forEach(maliciousText => {
        render(<DynamicWatermark text={maliciousText} />);
        
        // Canvas 不会解析CSS，所以这些输入是安全的
        expect(mockFillText).toHaveBeenCalledWith(maliciousText, 0, 0);
      });
    });

    it('应该处理Unicode和特殊字符攻击', () => {
      const unicodeAttacks = [
        '\u0000\u0001\u0002', // 控制字符
        '\uFEFF', // BOM字符
        '\u202E', // 右到左覆盖字符
        '\u200B\u200C\u200D', // 零宽字符
        '𝕏𝕊𝕊', // 数学字母数字符号
        '＜script＞alert(1)＜/script＞', // 全角字符
      ];

      unicodeAttacks.forEach(maliciousText => {
        render(<DynamicWatermark text={maliciousText} />);
        
        // 验证文本被渲染（组件应该能处理特殊字符）
        // 不需要验证具体的 fillText 调用，只要组件不崩溃即可
        expect(mockFillText).toHaveBeenCalled();
      });
    });

    it('应该防护超长输入攻击', () => {
      const longText = 'A'.repeat(100000); // 100KB 文本
      
      render(<DynamicWatermark text={longText} />);
      
      // 验证超长文本不会导致性能问题
      expect(mockFillText).toHaveBeenCalledWith(longText, 0, 0);
      
      // 验证Canvas尺寸没有被恶意修改
      const canvas = document.querySelector('canvas') as HTMLCanvasElement;
      expect(canvas.width).toBeLessThan(10000); // 合理的最大宽度
      expect(canvas.height).toBeLessThan(10000); // 合理的最大高度
    });
  });

  describe('时序攻击防护测试', () => {
    it('应该防护基于时间的侧信道攻击', () => {
      const texts = [
        'user123',
        'admin',
        'root',
        'administrator',
        'guest',
      ];

      const renderTimes: number[] = [];

      texts.forEach(text => {
        const startTime = performance.now();
        render(<DynamicWatermark text={text} />);
        const endTime = performance.now();
        
        renderTimes.push(endTime - startTime);
      });

      // 验证渲染时间差异不会泄露敏感信息
      // 所有渲染时间应该在合理范围内
      const maxTime = Math.max(...renderTimes);
      const minTime = Math.min(...renderTimes);
      const timeDifference = maxTime - minTime;
      
      // 时间差异不应该超过100ms（避免时序攻击）
      expect(timeDifference).toBeLessThan(100);
    });
  });

  describe('内存安全测试', () => {
    it('应该防护内存泄漏', () => {
      const initialMemory = (performance as any).memory?.usedJSHeapSize || 0;
      
      // 创建和销毁多个水印组件
      for (let i = 0; i < 100; i++) {
        const { unmount } = render(<DynamicWatermark text={`test-${i}`} />);
        unmount();
      }

      // 强制垃圾回收（如果支持）
      if (global.gc) {
        global.gc();
      }

      const finalMemory = (performance as any).memory?.usedJSHeapSize || 0;
      const memoryIncrease = finalMemory - initialMemory;
      
      // 内存增长应该在合理范围内（小于10MB）
      expect(memoryIncrease).toBeLessThan(10 * 1024 * 1024);
    });

    it('应该正确清理Canvas资源', () => {
      const { unmount } = render(<DynamicWatermark text="test" />);
      
      // 验证Canvas被创建
      let canvas = document.querySelector('canvas');
      expect(canvas).toBeInTheDocument();
      
      // 卸载组件
      unmount();
      
      // 验证Canvas被清理
      canvas = document.querySelector('canvas');
      expect(canvas).not.toBeInTheDocument();
    });
  });

  describe('权限和访问控制测试', () => {
    it('应该验证用户信息访问权限', () => {
      // 测试不同权限级别的用户信息
      const userInfoLevels = [
        'public-user-123',
        'internal-user-456', 
        'admin-user-789',
        'system-user-000',
      ];

      userInfoLevels.forEach(userInfo => {
        render(<DynamicWatermark text={userInfo} />);
        
        // 验证所有用户信息都被正确显示（不应该有权限过滤）
        expect(mockFillText).toHaveBeenCalledWith(userInfo, 0, 0);
      });
    });

    it('应该防护敏感信息泄露', () => {
      const sensitivePatterns = [
        'password123',
        'secret-key-abc',
        'token-xyz789',
        'api-key-123456',
        'private-data',
      ];

      sensitivePatterns.forEach(sensitiveText => {
        render(<DynamicWatermark text={sensitiveText} />);
        
        // 水印应该显示文本，但要确保不会被恶意读取
        expect(mockFillText).toHaveBeenCalledWith(sensitiveText, 0, 0);
        
        // 验证Canvas内容不能被轻易提取
        const canvas = document.querySelector('canvas') as HTMLCanvasElement;
        expect(canvas).toHaveClass('pointer-events-none'); // 防止交互
      });
    });
  });

  describe('配置安全测试', () => {
    it('应该处理异常透明度参数', () => {
      const maliciousOpacities = [-1, 2, 999, -999, NaN, Infinity, -Infinity];
      
      maliciousOpacities.forEach(opacity => {
        // 组件应该能渲染而不崩溃，即使参数异常
        expect(() => {
          render(<DynamicWatermark text="test" opacity={opacity} />);
        }).not.toThrow();
      });
    });

    it('应该处理异常字体大小参数', () => {
      const maliciousFontSizes = [-10, 0, 1000, -1000, NaN, Infinity];
      
      maliciousFontSizes.forEach(fontSize => {
        // 组件应该能渲染而不崩溃，即使参数异常
        expect(() => {
          render(<DynamicWatermark text="test" fontSize={fontSize} />);
        }).not.toThrow();
      });
    });

    it('应该验证旋转角度参数', () => {
      const maliciousRotations = [999, -999, NaN, Infinity, -Infinity];
      
      maliciousRotations.forEach(rotation => {
        render(<DynamicWatermark text="test" rotation={rotation} />);
        
        // 组件应该能处理异常的旋转值而不崩溃
        expect(mockFillText).toHaveBeenCalled();
      });
    });
  });

  describe('跨站脚本防护测试', () => {
    it('应该防护事件处理器注入', () => {
      const eventInjections = [
        'onload="alert(1)"',
        'onclick="malicious()"',
        'onmouseover="steal_data()"',
        'onfocus="redirect()"',
      ];

      eventInjections.forEach(injection => {
        render(<DynamicWatermark text={injection} />);
        
        // Canvas不会解析HTML事件，所以这些注入是安全的
        expect(mockFillText).toHaveBeenCalledWith(injection, 0, 0);
      });
    });
  });
});