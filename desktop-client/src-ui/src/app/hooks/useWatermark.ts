import { useState, useEffect } from 'react';
import { sessionApi } from '../utils/tauri';
import { tracing } from '@utils/tracing';

export interface WatermarkConfig {
  enabled: boolean;
  text: string;
  opacity: number;
  fontSize: number;
  rotation: number;
  spacing: number;
}

/**
 * 水印管理 Hook
 * 
 * 管理水印的配置和用户信息
 */
export function useWatermark() {
  const [config, setConfig] = useState<WatermarkConfig>({
    enabled: true,
    text: '',
    opacity: 0.1,
    fontSize: 16,
    rotation: -20,
    spacing: 200,
  });

  const [loading, setLoading] = useState(true);

  // 获取用户信息并生成水印文本
  useEffect(() => {
    const loadUserInfo = async () => {
      try {
        setLoading(true);
        
        // 尝试获取会话信息
        const sessionInfo = await sessionApi.getSessionInfo();
        
        // 生成水印文本：用户ID + 当前时间
        const now = new Date();
        const timeStr = now.toLocaleString('zh-CN', {
          year: 'numeric',
          month: '2-digit',
          day: '2-digit',
          hour: '2-digit',
          minute: '2-digit',
        });
        
        const watermarkText = `${sessionInfo.user_id} | ${timeStr}`;
        
        setConfig(prev => ({
          ...prev,
          text: watermarkText,
        }));
        
        tracing.info('Watermark initialized', { text: watermarkText });
      } catch (err) {
        tracing.warn('Failed to load user info for watermark, using fallback', { error: err });
        
        // 如果无法获取用户信息，使用默认文本
        const now = new Date();
        const timeStr = now.toLocaleString('zh-CN', {
          year: 'numeric',
          month: '2-digit',
          day: '2-digit',
          hour: '2-digit',
          minute: '2-digit',
        });
        
        const fallbackText = `Desktop Client | ${timeStr}`;
        
        setConfig(prev => ({
          ...prev,
          text: fallbackText,
        }));
      } finally {
        setLoading(false);
      }
    };

    loadUserInfo();
  }, []);

  // 更新水印配置
  const updateConfig = (newConfig: Partial<WatermarkConfig>) => {
    setConfig(prev => ({ ...prev, ...newConfig }));
  };

  // 切换水印启用状态
  const toggleEnabled = () => {
    setConfig(prev => ({ ...prev, enabled: !prev.enabled }));
  };

  // 刷新水印文本（更新时间）
  const refreshText = async () => {
    try {
      const sessionInfo = await sessionApi.getSessionInfo();
      const now = new Date();
      const timeStr = now.toLocaleString('zh-CN', {
        year: 'numeric',
        month: '2-digit',
        day: '2-digit',
        hour: '2-digit',
        minute: '2-digit',
      });
      
      const watermarkText = `${sessionInfo.user_id} | ${timeStr}`;
      
      setConfig(prev => ({
        ...prev,
        text: watermarkText,
      }));
    } catch (err) {
      tracing.warn('Failed to refresh watermark text', { error: err });
    }
  };

  return {
    config,
    loading,
    updateConfig,
    toggleEnabled,
    refreshText,
  };
}