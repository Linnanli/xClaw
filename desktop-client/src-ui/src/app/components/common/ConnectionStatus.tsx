import { useState, useEffect } from 'react';
import { Wifi, WifiOff, RotateCcw } from 'lucide-react';
import { useTheme } from '../../contexts/ThemeContext';
import { sseService, ConnectionStatus as ConnectionStatusType } from '../../services/sseService';

/**
 * 连接状态指示器组件
 * 
 * 显示SSE连接的实时状态，包括：
 * - 连接状态指示灯
 * - 状态文本
 * - 连接图标
 */
export function ConnectionStatus() {
  const { theme } = useTheme();
  const [status, setStatus] = useState<ConnectionStatusType>(sseService.getConnectionStatus());

  useEffect(() => {
    // 监听连接状态变化
    const handleStatusChange = (newStatus: ConnectionStatusType) => {
      setStatus(newStatus);
    };

    sseService.onStatusChange(handleStatusChange);

    // 清理监听器
    return () => {
      sseService.offStatusChange(handleStatusChange);
    };
  }, []);

  const getStatusConfig = () => {
    switch (status) {
      case 'connected':
        return {
          text: '已连接',
          dotColor: 'bg-green-400',
          icon: <Wifi size={16} className="text-green-400" />,
          animate: false
        };
      case 'connecting':
        return {
          text: '连接中',
          dotColor: 'bg-yellow-400',
          icon: <RotateCcw size={16} className="text-yellow-400 animate-spin" />,
          animate: true
        };
      case 'reconnecting':
        return {
          text: '重连中',
          dotColor: 'bg-yellow-400',
          icon: <RotateCcw size={16} className="text-yellow-400 animate-spin" />,
          animate: true
        };
      case 'disconnected':
        return {
          text: '已断开',
          dotColor: 'bg-red-400',
          icon: <WifiOff size={16} className="text-red-400" />,
          animate: false
        };
      case 'failed':
        return {
          text: '连接失败',
          dotColor: 'bg-red-400',
          icon: <WifiOff size={16} className="text-red-400" />,
          animate: false
        };
      default:
        return {
          text: '未知状态',
          dotColor: 'bg-gray-400',
          icon: <WifiOff size={16} className="text-gray-400" />,
          animate: false
        };
    }
  };

  const config = getStatusConfig();

  return (
    <div className="flex items-center gap-2">
      {/* 状态指示灯 */}
      <div className="relative">
        <div 
          data-testid="connection-dot"
          className={`w-3 h-3 rounded-full ${config.dotColor} ${
            config.animate ? 'animate-pulse' : ''
          }`}
        />
        {/* 连接成功时的光晕效果 */}
        {status === 'connected' && (
          <div className="absolute inset-0 w-3 h-3 rounded-full bg-green-400 animate-ping opacity-20" />
        )}
      </div>

      {/* 连接图标 */}
      {config.icon}

      {/* 状态文本 */}
      <span 
        className={`text-sm font-medium ${
          theme === 'dark' ? 'text-white' : 'text-[#333]'
        }`}
      >
        {config.text}
      </span>
    </div>
  );
}