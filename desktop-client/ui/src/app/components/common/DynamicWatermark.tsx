import { useEffect, useRef, useState } from 'react';
import { useTheme } from '../../contexts/ThemeContext';

export interface DynamicWatermarkProps {
  text: string;
  enabled?: boolean;
  opacity?: number;
  fontSize?: number;
  color?: string;
  rotation?: number;
  spacing?: number;
}

/**
 * 动态水印组件
 * 
 * 使用 Canvas 实现的动态水印，支持旋转显示和自定义样式
 */
export function DynamicWatermark({
  text,
  enabled = true,
  opacity = 0.1,
  fontSize = 16,
  color,
  rotation = -20,
  spacing = 200,
}: DynamicWatermarkProps) {
  const { theme } = useTheme();
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [dimensions, setDimensions] = useState({ width: 0, height: 0 });

  // 根据主题自动选择颜色
  const watermarkColor = color || (theme === 'dark' ? '#ffffff' : '#000000');

  // 监听窗口大小变化
  useEffect(() => {
    const updateDimensions = () => {
      setDimensions({
        width: window.innerWidth,
        height: window.innerHeight,
      });
    };

    updateDimensions();
    window.addEventListener('resize', updateDimensions);
    return () => window.removeEventListener('resize', updateDimensions);
  }, []);

  // 绘制水印
  useEffect(() => {
    if (!enabled || !canvasRef.current || !text.trim()) return;

    const canvas = canvasRef.current;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    // 设置 Canvas 尺寸
    canvas.width = dimensions.width;
    canvas.height = dimensions.height;

    // 清空画布
    ctx.clearRect(0, 0, canvas.width, canvas.height);

    // 设置文字样式
    ctx.font = `${fontSize}px Arial, sans-serif`;
    ctx.fillStyle = watermarkColor;
    ctx.globalAlpha = opacity;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';

    // 计算文字尺寸
    const textMetrics = ctx.measureText(text);
    const textWidth = textMetrics.width;
    const textHeight = fontSize;

    // 计算网格布局
    const cols = Math.ceil(canvas.width / spacing) + 2;
    const rows = Math.ceil(canvas.height / spacing) + 2;

    // 绘制水印网格
    for (let row = 0; row < rows; row++) {
      for (let col = 0; col < cols; col++) {
        const x = col * spacing - spacing / 2;
        const y = row * spacing - spacing / 2;

        ctx.save();
        ctx.translate(x, y);
        ctx.rotate((rotation * Math.PI) / 180);
        ctx.fillText(text, 0, 0);
        ctx.restore();
      }
    }
  }, [enabled, text, dimensions, watermarkColor, opacity, fontSize, rotation, spacing]);

  if (!enabled || !text.trim()) {
    return null;
  }

  return (
    <canvas
      ref={canvasRef}
      className="fixed inset-0 pointer-events-none z-50"
      style={{
        width: '100%',
        height: '100%',
      }}
    />
  );
}