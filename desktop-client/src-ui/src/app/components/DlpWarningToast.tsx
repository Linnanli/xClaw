/**
 * DLP 警告 Toast 组件
 * 当检测到敏感信息时显示提示
 */

import React from 'react';

interface DlpWarningToastProps {
  redactedCount: number;
  blockedCount: number;
  onClose: () => void;
}

export const DlpWarningToast: React.FC<DlpWarningToastProps> = ({
  redactedCount,
  blockedCount,
  onClose,
}) => {
  return (
    <div className="fixed top-4 right-4 z-50 animate-slide-in">
      <div className="bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800 rounded-lg shadow-lg p-4 max-w-md">
        <div className="flex items-start gap-3">
          <div className="flex-shrink-0">
            <svg className="w-6 h-6 text-yellow-600 dark:text-yellow-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
            </svg>
          </div>
          
          <div className="flex-1">
            <h3 className="text-sm font-medium text-yellow-800 dark:text-yellow-200">
              检测到敏感信息
            </h3>
            <div className="mt-1 text-sm text-yellow-700 dark:text-yellow-300">
              {redactedCount > 0 && (
                <p>已脱敏 {redactedCount} 处敏感信息</p>
              )}
              {blockedCount > 0 && (
                <p>已阻止 {blockedCount} 处高危信息</p>
              )}
            </div>
          </div>
          
          <button
            onClick={onClose}
            className="flex-shrink-0 text-yellow-600 dark:text-yellow-400 hover:text-yellow-800 dark:hover:text-yellow-200"
          >
            <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
      </div>
    </div>
  );
};
