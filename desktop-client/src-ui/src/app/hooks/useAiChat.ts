/**
 * AI Chat Hook - 与后端 SSE 端点集成
 * 直接调用后端 API，而不是使用 Vercel AI SDK 的 useChat Hook
 */

import { useState, useCallback, useRef } from 'react';

interface Message {
  id: string;
  role: 'user' | 'assistant';
  content: string;
}

interface UseAiChatOptions {
  threadId: string;
  apiUrl?: string;
  authToken?: string;
}

export function useAiChat(options: UseAiChatOptions) {
  const { threadId, apiUrl = 'http://localhost:3000', authToken = '' } = options;
  
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  
  const messageIdRef = useRef(0);

  // 发送消息
  const append = useCallback(
    async (message: { role: 'user' | 'assistant'; content: string }) => {
      try {
        setError(null);
        setIsLoading(true);
        
        // 添加用户消息到本地状态
        const userMessage: Message = {
          id: `msg-${messageIdRef.current++}`,
          role: message.role,
          content: message.content,
        };
        
        setMessages(prev => [...prev, userMessage]);
        
        // 发送消息到后端
        const response = await fetch(`${apiUrl}/api/chat/send`, {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            'Authorization': `Bearer ${authToken}`,
          },
          body: JSON.stringify({
            content: message.content,
            thread_id: threadId,
          }),
        });
        
        if (!response.ok) {
          const errorText = await response.text();
          let errorMessage = `HTTP ${response.status}: ${response.statusText}`;
          
          // 尝试解析错误响应
          try {
            const errorData = JSON.parse(errorText);
            if (errorData.error) {
              errorMessage = errorData.error;
            } else if (errorData.message) {
              errorMessage = errorData.message;
            }
          } catch {
            // 如果不是 JSON，使用原始文本
            if (errorText) {
              errorMessage = errorText;
            }
          }
          
          throw new Error(errorMessage);
        }
        
        const data = await response.json();
        console.log('📤 Message sent:', data);
        
        // 添加助手消息到本地状态
        const assistantMessage: Message = {
          id: data.message_id || `msg-${messageIdRef.current++}`,
          role: 'assistant',
          content: 'Processing your message...',
        };
        
        setMessages(prev => [...prev, assistantMessage]);
      } catch (err) {
        const errorMsg = err instanceof Error ? err.message : 'Unknown error';
        console.error('❌ Failed to send message:', errorMsg);
        setError(err instanceof Error ? err : new Error(errorMsg));
        
        // 移除失败的用户消息
        setMessages(prev => prev.slice(0, -1));
      } finally {
        setIsLoading(false);
      }
    },
    [threadId, apiUrl, authToken]
  );

  // 重新加载消息
  const reload = useCallback(async () => {
    if (messages.length === 0) return;
    
    const lastUserMessage = [...messages].reverse().find(m => m.role === 'user');
    if (lastUserMessage) {
      setMessages(prev => prev.filter(m => m.id !== lastUserMessage.id));
      await append({ role: 'user', content: lastUserMessage.content });
    }
  }, [messages, append]);

  // 停止生成
  const stop = useCallback(() => {
    setIsLoading(false);
  }, []);

  return {
    messages,
    input,
    setInput,
    isLoading,
    error,
    append,
    reload,
    stop,
    handleSubmit: (e: React.FormEvent) => {
      e.preventDefault();
      if (input.trim()) {
        append({ role: 'user', content: input });
        setInput('');
      }
    },
  };
}
