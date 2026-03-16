/**
 * AI Chat Hook - 与后端 SSE 端点集成
 * 直接调用后端 API，并通过 SSE 接收响应
 */

import { useState, useCallback, useRef, useEffect } from 'react';
import { createSseClient, type SseClient, type SseEvent } from '../utils/sse';

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
  const sseClientRef = useRef<SseClient | null>(null);
  const currentAssistantMessageIdRef = useRef<string | null>(null);

  // 初始化 SSE 连接
  useEffect(() => {
    if (!authToken) {
      console.log('⚠️  No auth token, skipping SSE connection');
      return;
    }

    const initSse = async () => {
      try {
        console.log('🔗 Initializing SSE connection...');
        console.log('   API URL:', apiUrl);
        console.log('   Auth Token:', authToken.substring(0, 20) + '...');
        console.log('   Thread ID:', threadId || '(none)');
        
        const client = createSseClient(apiUrl, authToken);
        
        // 注册事件处理器
        client.onEvent((event: SseEvent) => {
          console.log('📨 SSE Event:', event.type, event.data);
          
          if (event.type === 'message') {
            // 新消息
            const msg = event.data;
            // 只处理当前线程的消息（如果有 threadId）
            if (!threadId || msg.thread_id === threadId) {
              setMessages(prev => {
                // 检查消息是否已存在
                const exists = prev.some(m => m.id === msg.message_id);
                if (exists) return prev;
                
                return [...prev, {
                  id: msg.message_id,
                  role: msg.role as 'user' | 'assistant',
                  content: msg.content,
                }];
              });
            }
          } else if (event.type === 'message_update') {
            // 消息更新（流式响应）
            const update = event.data;
            currentAssistantMessageIdRef.current = update.message_id;
            
            setMessages(prev => {
              const index = prev.findIndex(m => m.id === update.message_id);
              if (index >= 0) {
                // 更新现有消息
                const newMessages = [...prev];
                newMessages[index] = {
                  ...newMessages[index],
                  content: update.content,
                };
                return newMessages;
              } else {
                // 创建新消息
                return [...prev, {
                  id: update.message_id,
                  role: 'assistant',
                  content: update.content,
                }];
              }
            });
          } else if (event.type === 'thread_state') {
            // 线程状态更新
            const state = event.data;
            // 只处理当前线程的状态（如果有 threadId）
            if (!threadId || state.thread_id === threadId) {
              if (state.state === 'completed') {
                setIsLoading(false);
                currentAssistantMessageIdRef.current = null;
              }
            }
          }
        });
        
        // 连接 SSE
        await client.connect();
        sseClientRef.current = client;
        console.log('✅ SSE connected');
      } catch (err) {
        console.error('❌ Failed to connect SSE:', err);
      }
    };

    initSse();

    // 清理函数
    return () => {
      if (sseClientRef.current) {
        console.log('🔌 Disconnecting SSE...');
        sseClientRef.current.disconnect();
        sseClientRef.current = null;
      }
    };
  }, [apiUrl, authToken]); // 移除 threadId 依赖，SSE 连接应该一直保持

  // 发送消息
  const append = useCallback(
    async (message: { role: 'user' | 'assistant'; content: string }) => {
      try {
        console.log('📤 Sending message...');
        console.log('   Thread ID:', threadId);
        console.log('   Content:', message.content);
        console.log('   SSE Client:', sseClientRef.current ? 'Connected' : 'Not connected');
        
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
        const url = `${apiUrl}/api/chat/send`;
        console.log('   POST URL:', url);
        
        const response = await fetch(url, {
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
        
        console.log('   Response status:', response.status);
        
        if (!response.ok) {
          const errorText = await response.text();
          console.error('   Response error:', errorText);
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
        console.log('✅ Message sent successfully:', data);
        
        // SSE 会自动接收助手的响应，不需要手动添加
      } catch (err) {
        const errorMsg = err instanceof Error ? err.message : 'Unknown error';
        console.error('❌ Failed to send message:', errorMsg);
        setError(err instanceof Error ? err : new Error(errorMsg));
        setIsLoading(false);
        
        // 移除失败的用户消息
        setMessages(prev => prev.slice(0, -1));
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
    setMessages, // 暴露 setMessages 用于初始化消息
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
