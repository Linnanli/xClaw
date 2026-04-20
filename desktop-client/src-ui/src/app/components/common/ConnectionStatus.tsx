import { useState, useEffect, useRef } from 'react';
import { Cpu, AlertCircle } from 'lucide-react';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { useTheme } from '../../contexts/ThemeContext';

type EngineStatus = 'starting' | 'running' | 'error';

/**
 * 引擎状态指示器 — 嵌入式模式
 *
 * 监听 Tauri IPC `chat-stream` 判断引擎是否就绪。
 * 收到任何非 Error 事件即视为引擎运行中。
 */
export function ConnectionStatus() {
  const { theme } = useTheme();
  const [status, setStatus] = useState<EngineStatus>('starting');
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const unlistenRef = useRef<UnlistenFn | null>(null);

  useEffect(() => {
    let cancelled = false;

    // 监听引擎事件判断状态
    listen<any>('chat-stream', (event) => {
      const payload = event.payload;
      // VercelUIStream::Error → { type: "error", errorText: "..." }
      if (payload?.type === 'error') {
        const errorText = payload.errorText as string | undefined;
        if (errorText) {
          setStatus('error');
          setErrorMsg(errorText);
          return;
        }
      }
      // 收到任何非错误事件 → 引擎运行中
      setStatus('running');
    }).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlistenRef.current = fn;
      }
    });

    // 5 秒后如果还在 starting，也切换到 running（引擎可能已就绪但无事件）
    const timer = setTimeout(() => {
      setStatus((prev) => (prev === 'starting' ? 'running' : prev));
    }, 5000);

    return () => {
      cancelled = true;
      unlistenRef.current?.();
      clearTimeout(timer);
    };
  }, []);

  const config = (() => {
    switch (status) {
      case 'running':
        return {
          text: '引擎运行中',
          dotColor: 'bg-green-400',
          icon: <Cpu size={16} className="text-green-400" />,
          animate: false,
        };
      case 'starting':
        return {
          text: '引擎启动中',
          dotColor: 'bg-yellow-400',
          icon: <Cpu size={16} className="text-yellow-400 animate-spin" />,
          animate: true,
        };
      case 'error':
        return {
          text: '引擎异常',
          dotColor: 'bg-red-400',
          icon: <AlertCircle size={16} className="text-red-400" />,
          animate: false,
        };
    }
  })();

  return (
    <div className="flex items-center gap-2" title={errorMsg || undefined}>
      <div className="relative">
        <div
          data-testid="connection-dot"
          className={`w-3 h-3 rounded-full ${config.dotColor} ${
            config.animate ? 'animate-pulse' : ''
          }`}
        />
        {status === 'running' && (
          <div className="absolute inset-0 w-3 h-3 rounded-full bg-green-400 animate-ping opacity-20" />
        )}
      </div>
      {config.icon}
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
