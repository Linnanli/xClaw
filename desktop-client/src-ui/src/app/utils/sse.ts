// Server-Sent Events (SSE) client for real-time updates

export interface SSEOptions {
  url: string;
  onMessage: (data: any) => void;
  onError?: (error: Error) => void;
  onOpen?: () => void;
  onClose?: () => void;
}

export class SSEClient {
  private eventSource: EventSource | null = null;
  private options: SSEOptions;
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 5;
  private reconnectDelay = 1000;

  constructor(options: SSEOptions) {
    this.options = options;
  }

  connect() {
    try {
      this.eventSource = new EventSource(this.options.url);

      this.eventSource.onopen = () => {
        this.reconnectAttempts = 0;
        this.options.onOpen?.();
      };

      this.eventSource.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data);
          this.options.onMessage(data);
        } catch (err) {
          console.error('Failed to parse SSE message:', err);
        }
      };

      this.eventSource.onerror = (error) => {
        console.error('SSE error:', error);
        this.handleError(error as Error);
      };
    } catch (err) {
      this.handleError(err as Error);
    }
  }

  disconnect() {
    if (this.eventSource) {
      this.eventSource.close();
      this.eventSource = null;
    }
  }

  private handleError(error: Error) {
    this.options.onError?.(error);

    // Attempt to reconnect
    if (this.reconnectAttempts < this.maxReconnectAttempts) {
      this.reconnectAttempts++;
      const delay = this.reconnectDelay * Math.pow(2, this.reconnectAttempts - 1);
      console.log(`Reconnecting in ${delay}ms...`);
      setTimeout(() => this.connect(), delay);
    } else {
      this.options.onClose?.();
    }
  }

  isConnected(): boolean {
    return this.eventSource !== null && this.eventSource.readyState === EventSource.OPEN;
  }
}

// Log streaming client
export class LogStreamClient extends SSEClient {
  constructor(baseUrl: string, onLogEntry: (entry: any) => void) {
    super({
      url: `${baseUrl}/api/logs/events`,
      onMessage: onLogEntry,
      onOpen: () => console.log('Log stream connected'),
      onClose: () => console.log('Log stream closed'),
      onError: (err) => console.error('Log stream error:', err),
    });
  }
}

// Chat events streaming client
export class ChatStreamClient extends SSEClient {
  constructor(baseUrl: string, threadId: string, onEvent: (event: any) => void) {
    super({
      url: `${baseUrl}/api/chat/threads/${threadId}/events`,
      onMessage: onEvent,
      onOpen: () => console.log('Chat stream connected'),
      onClose: () => console.log('Chat stream closed'),
      onError: (err) => console.error('Chat stream error:', err),
    });
  }
}
