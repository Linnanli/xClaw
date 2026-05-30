/**
 * 轻量级日志追踪工具
 * 
 * 提供结构化日志记录，便于调试和审计
 */

export type LogLevel = 'debug' | 'info' | 'warn' | 'error';

export interface LogContext {
  [key: string]: any;
}

class TracingService {
  private isDevelopment: boolean;

  constructor() {
    this.isDevelopment = import.meta.env.DEV;
  }

  /**
   * 记录调试信息
   */
  debug(message: string, context?: LogContext): void {
    if (this.isDevelopment) {
      this.log('debug', message, context);
    }
  }

  /**
   * 记录一般信息
   */
  info(message: string, context?: LogContext): void {
    this.log('info', message, context);
  }

  /**
   * 记录警告信息
   */
  warn(message: string, context?: LogContext): void {
    this.log('warn', message, context);
  }

  /**
   * 记录错误信息
   */
  error(message: string, context?: LogContext): void {
    this.log('error', message, context);
  }

  /**
   * 内部日志记录方法
   */
  private log(level: LogLevel, message: string, context?: LogContext): void {
    const timestamp = new Date().toISOString();
    const logEntry = {
      timestamp,
      level,
      message,
      ...(context && { context })
    };

    // 根据日志级别选择console方法
    switch (level) {
      case 'debug':
        console.debug(`[${timestamp}] DEBUG: ${message}`, context || '');
        break;
      case 'info':
        console.info(`[${timestamp}] INFO: ${message}`, context || '');
        break;
      case 'warn':
        console.warn(`[${timestamp}] WARN: ${message}`, context || '');
        break;
      case 'error':
        console.error(`[${timestamp}] ERROR: ${message}`, context || '');
        break;
    }

    // 在生产环境中，可以将日志发送到远程服务
    if (!this.isDevelopment) {
      this.sendToRemoteLogging(logEntry);
    }
  }

  /**
   * 发送日志到远程服务（生产环境）
   */
  private sendToRemoteLogging(logEntry: any): void {
    // TODO: 实现远程日志发送
    // 可以发送到后端的日志收集API
  }
}

export const tracing = new TracingService();