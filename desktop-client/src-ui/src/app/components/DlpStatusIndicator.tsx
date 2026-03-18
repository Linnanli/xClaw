/**
 * DLP 状态指示器组件
 * 显示 DLP 功能是否启用
 */

import React, { useState, useEffect } from 'react';
import { useDlpScan } from '../hooks/useDlpScan';

export const DlpStatusIndicator: React.FC = () => {
  const [enabled, setEnabled] = useState(true);
  const [loading, setLoading] = useState(true);
  const { getDlpConfig } = useDlpScan();

  useEffect(() => {
    const loadConfig = async () => {
      try {
        const config = await getDlpConfig();
        setEnabled(config.enabled);
      } catch (error) {
        console.error('Failed to load DLP config:', error);
      } finally {
        setLoading(false);
      }
    };

    loadConfig();
  }, [getDlpConfig]);

  if (loading) {
    return null;
  }

  return (
    <div 
      className="flex items-center gap-2 px-3 py-1.5 rounded-full text-xs font-medium transition-colors cursor-help"
      style={{
        backgroundColor: enabled 
          ? 'rgba(34, 197, 94, 0.1)' 
          : 'rgba(156, 163, 175, 0.1)',
        color: enabled 
          ? 'rgb(34, 197, 94)' 
          : 'rgb(156, 163, 175)',
      }}
      title={enabled ? 'DLP 数据保护已启用' : 'DLP 数据保护已禁用'}
    >
      <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
      </svg>
      <span>DLP {enabled ? '已启用' : '已禁用'}</span>
    </div>
  );
};
