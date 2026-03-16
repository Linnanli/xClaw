import { useState, useEffect, useRef } from 'react';
import { Plus, ChevronLeft, ChevronRight, Image, Send, Wifi, WifiOff } from 'lucide-react';
import { useTheme } from '../../contexts/ThemeContext';
import { threadApi, type Thread, type Message } from '../../utils/tauri';
import { createSseClient, type SseEvent, type SseClient } from '../../utils/sse';

interface PerformanceMetrics {
  messagesReceived: number;
  eventsProcessed: number;
  lastEventTime: number;
  averageLatency: number;
}

export function ChatTabWithSSE() {
  const { theme } = useTheme();
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [selectedConversation, setSelectedConversation] = useState<string | null>(null);
  const [inputText, setInputText] = useState('');
  const [conversations, setConversations] = useState<Thread[]>([]);
  const [messages, setMessages] = useState<Message[]>([]);
  const [loading, setLoading] = useState(false);
  
  // SSE 相关状态
  const [sseClient, setSseClient] = useState<SseClient | null>(null);
  const [sseConnected, setSseConnected] = useState(false);
  const [sseReconnecting, setSseReconnecting] = useState(false);
  const [sseError, setSseError] = useState<string | null>(null);
  
  // 性能监控
  const [metrics, setMetrics] = useState<PerformanceMetrics>({
    messagesReceived: 0,
    eventsProcessed: 0,
    lastEventTime: 0,
    averageLatency: 0,
  });
  
  // 去重和批量更新
  const processedMessageIds = useRef(new Set<string>());
  const pendingMessages = useRef<Message[]>([]);
  const updateTimer = useRef<NodeJS.Timeout | null>(null);
  const reconnectAttempts = useRef(0);
  const maxReconnectAttempts = 5;
  const reconnectDelay = useRef(1000);

  useEffect(() => {
    loadConversations();
  }, []);

  useEffect(() => {
    if (selectedConversation) {
      loadMessages(selectedConversation);
      connectSSE(selectedConversation);
    }
  }, [selectedConversation]);

  const loadConversations = async () => {
    try {
      const response = await threadApi.getThreads();
      const threads = Array.isArray(response) ? response : (response.threads || []);
      setConversations(threads);
      
      if (threads.length === 0) {
        try {
          const newThread = await threadApi.createThread();
          setConversations([newThread]);
          setSelectedConversation(newThread.id);
        } catch (err) {
          console.error('Failed to create initial thread:', err);
        }
      } else if (!selectedConversation) {
        setSelectedConversation(threads[0].id);
      }
    } catch (err) {
      console.error('Failed to load conversations:', err);
    }
  };

  const loadMessages = async (threadId: string) => {
    try {
      const msgs = await threadApi.getMessages(threadId);
      setMessages(msgs);
      processedMessageIds.current.clear();
      msgs.forEach(msg => processedMessageIds.current.add(msg.id));
    } catch (err) {
      console.error('Failed to load messages:', err);
    }
  };

  const connectSSE = async (threadId: string) => {
    try {
      setSseError(null);
      setSseReconnecting(false);
      reconnectAttempts.current = 0;
      reconnectDelay.current = 1000;
      
      const client = createSseClient(
        'http://localhost:3000',
        '8a7f756f4179fb10a79e58a512968ad8bfb545f875524ab2ed8ce0333a2030a4'
      );
      
      // 注册事件处理器
      client.onEvent((event: SseEvent) => {
        handleSseEvent(event, threadId);
      });
      
      // 连接到 SSE 流
      await client.connect();
      setSseConnected(true);
      setSseClient(client);
      setSseError(null);
    } catch (err) {
      console.error('Failed to connect to SSE:', err);
      setSseConnected(false);
      setSseError(err instanceof Error ? err.message : 'SSE connection failed');
      attemptReconnect(threadId);
    }
  };

  const attemptReconnect = (threadId: string) => {
    if (reconnectAttempts.current < maxReconnectAttempts) {
      setSseReconnecting(true);
      reconnectAttempts.current++;
      
      setTimeout(() => {
        connectSSE(threadId);
      }, reconnectDelay.current);
      
      reconnectDelay.current = Math.min(reconnectDelay.current * 2, 30000);
    } else {
      setSseError('Failed to reconnect after multiple attempts');
    }
  };

  const handleSseEvent = (event: SseEvent, threadId: string) => {
    const now = Date.now();
    
    if (event.type === 'message') {
      // 消息去重
      if (!processedMessageIds.current.has(event.data.message_id)) {
        processedMessageIds.current.add(event.data.message_id);
        
        const newMessage: Message = {
          id: event.data.message_id,
          thread_id: event.data.thread_id,
          role: event.data.role,
          content: event.data.content,
          created_at: new Date().toISOString(),
        };
        
        pendingMessages.current.push(newMessage);
        
        // 批量更新
        if (!updateTimer.current) {
          updateTimer.current = setTimeout(() => {
            setMessages(prev => {
              const updated = [...prev, ...pendingMessages.current];
              // 限制消息历史大小
              if (updated.length > 1000) {
                return updated.slice(-1000);
              }
              return updated;
            });
            
            // 更新性能指标
            setMetrics(prev => ({
              ...prev,
              messagesReceived: prev.messagesReceived + pendingMessages.current.length,
              eventsProcessed: prev.eventsProcessed + 1,
              lastEventTime: now,
              averageLatency: (prev.averageLatency + (now - prev.lastEventTime)) / 2,
            }));
            
            pendingMessages.current = [];
            updateTimer.current = null;
          }, 100);
        }
      }
    } else if (event.type === 'thread_state') {
      console.log('Thread state changed:', event.data.state);
      // 更新对话状态
      setConversations(prev =>
        prev.map(conv =>
          conv.id === event.data.thread_id
            ? { ...conv, state: event.data.state }
            : conv
        )
      );
    } else if (event.type === 'auth_completed') {
      console.log('Auth completed:', event.data.extension_name);
    } else if (event.type === 'auth_required') {
      console.log('Auth required:', event.data.extension_name);
    }
  };

  const handleCreateNew = async () => {
    try {
      const newThread = await threadApi.createThread();
      setConversations([newThread, ...conversations]);
      setSelectedConversation(newThread.id);
      setMessages([]);
    } catch (err) {
      console.error('Failed to create thread:', err);
    }
  };

  const handleSend = async () => {
    if (!inputText.trim()) return;
    
    if (!selectedConversation) {
      try {
        const newThread = await threadApi.createThread();
        setConversations([newThread, ...conversations]);
        setSelectedConversation(newThread.id);
        const content = inputText.trim();
        setInputText('');
        setLoading(true);
        try {
          await threadApi.sendMessage(newThread.id, content);
          // SSE 会自动接收新消息
        } catch (err) {
          console.error('Failed to send message:', err);
        } finally {
          setLoading(false);
        }
      } catch (err) {
        console.error('Failed to create thread:', err);
      }
      return;
    }
    
    const content = inputText.trim();
    setInputText('');
    setLoading(true);
    
    try {
      await threadApi.sendMessage(selectedConversation, content);
      // SSE 会自动接收新消息，无需轮询
    } catch (err) {
      console.error('Failed to send message:', err);
    } finally {
      setLoading(false);
    }
  };

  const formatTime = (dateStr: string) => {
    const date = new Date(dateStr);
    const now = new Date();
    const diff = now.getTime() - date.getTime();
    const minutes = Math.floor(diff / 60000);
    const hours = Math.floor(diff / 3600000);
    const days = Math.floor(diff / 86400000);

    if (minutes < 1) return '刚刚';
    if (minutes < 60) return `${minutes}分钟前`;
    if (hours < 24) return `${hours}小时前`;
    return `${days}天前`;
  };

  return (
    <div className="flex h-full">
      {/* Sidebar */}
      {sidebarOpen && (
        <div className={`w-64 border-r flex flex-col ${
          theme === 'dark'
            ? 'bg-[#0f1d35] border-[#1a2942]'
            : 'bg-white border-[#ddd]'
        }`}>
          <div className={`p-4 border-b ${
            theme === 'dark' ? 'border-[#1a2942]' : 'border-[#ddd]'
          }`}>
            <button 
              onClick={handleCreateNew}
              className={`w-full flex items-center justify-center gap-2 px-4 py-2 rounded-lg font-medium transition-opacity ${
              theme === 'dark'
                ? 'bg-gradient-to-r from-[#5ddad5] to-[#4facf7] text-[#0a1628] hover:opacity-90'
                : 'bg-[#667eea] text-white hover:opacity-90 shadow-md'
            }`}>
              <Plus size={18} />
              <span>新建对话</span>
            </button>
          </div>

          <div className={`p-4 border-b ${
            theme === 'dark' ? 'border-[#1a2942]' : 'border-[#ddd]'
          }`}>
            <label className={`block text-sm font-medium mb-2 ${
              theme === 'dark' ? 'text-white' : 'text-[#333]'
            }`}>
              选择助手
            </label>
            <select className={`w-full px-3 py-2 rounded-lg border focus:outline-none ${
              theme === 'dark'
                ? 'bg-[#0a1628] border-[#1a2942] focus:border-[#5ddad5] text-white'
                : 'bg-white border-[#ddd] focus:border-[#667eea] focus:ring-2 focus:ring-[#667eea]/20 text-[#333]'
            }`}>
              <option>默认助手</option>
              <option>编程助手</option>
              <option>写作助手</option>
            </select>
          </div>

          <div className="flex-1 overflow-y-auto">
            {conversations.map((conv) => (
              <button
                key={conv.id}
                onClick={() => setSelectedConversation(conv.id)}
                className={`w-full px-4 py-3 text-left border-b transition-colors ${
                  theme === 'dark'
                    ? `border-[#1a2942] hover:bg-[#0a1628] ${
                        selectedConversation === conv.id ? 'bg-[#0a1628] border-l-2 border-l-[#5ddad5]' : ''
                      }`
                    : `border-[#eee] hover:bg-[#f5f5f5] ${
                        selectedConversation === conv.id ? 'bg-[#f5f5f5] border-l-2 border-l-[#667eea]' : ''
                      }`
                }`}
              >
                <div className={`font-medium truncate ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>{conv.title}</div>
                <div className={`text-sm mt-1 ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>{formatTime(conv.updated_at)}</div>
              </button>
            ))}
          </div>
        </div>
      )}

      {/* Main Chat Area */}
      <div className={`flex-1 flex flex-col ${
        theme === 'dark' ? 'bg-[#0a1628]' : 'bg-[#f5f5f5]'
      }`}>
        {/* Header with SSE Status */}
        <div className={`p-2 border-b flex items-center justify-between ${
          theme === 'dark' ? 'border-[#1a2942]' : 'border-[#ddd]'
        }`}>
          <button
            onClick={() => setSidebarOpen(!sidebarOpen)}
            className={`p-2 rounded-lg transition-colors ${
              theme === 'dark'
                ? 'hover:bg-[#0f1d35] text-gray-400'
                : 'hover:bg-[#eee] text-[#666]'
            }`}
          >
            {sidebarOpen ? <ChevronLeft size={20} /> : <ChevronRight size={20} />}
          </button>
          
          {/* SSE Status Indicator */}
          <div className="flex items-center gap-2">
            {sseConnected ? (
              <div className="flex items-center gap-1 text-green-500">
                <Wifi size={16} />
                <span className="text-xs">SSE 已连接</span>
              </div>
            ) : sseReconnecting ? (
              <div className="flex items-center gap-1 text-yellow-500">
                <WifiOff size={16} />
                <span className="text-xs">重新连接中...</span>
              </div>
            ) : (
              <div className="flex items-center gap-1 text-red-500">
                <WifiOff size={16} />
                <span className="text-xs">SSE 未连接</span>
              </div>
            )}
          </div>
          
          {/* Performance Metrics */}
          <div className={`text-xs ${theme === 'dark' ? 'text-gray-400' : 'text-[#666]'}`}>
            消息: {metrics.messagesReceived} | 延迟: {metrics.averageLatency.toFixed(0)}ms
          </div>
        </div>

        {/* Error Message */}
        {sseError && (
          <div className="bg-red-100 border border-red-400 text-red-700 px-4 py-2 text-sm">
            {sseError}
          </div>
        )}

        {/* Messages */}
        <div className="flex-1 overflow-y-auto p-4 space-y-4">
          {messages.map((message) => (
            <div
              key={message.id}
              className={`flex ${
                message.role === 'user' ? 'justify-end' : message.role === 'system' ? 'justify-center' : 'justify-start'
              }`}
            >
              <div
                className={`max-w-2xl px-4 py-3 rounded-lg ${
                  message.role === 'user'
                    ? theme === 'dark'
                      ? 'bg-gradient-to-r from-[#5ddad5] to-[#4facf7] text-[#0a1628]'
                      : 'bg-[#667eea] text-white shadow-md'
                    : message.role === 'system'
                    ? theme === 'dark'
                      ? 'bg-yellow-400/10 text-yellow-400 border border-yellow-400/30'
                      : 'bg-yellow-50 text-yellow-700 border border-yellow-200'
                    : theme === 'dark'
                      ? 'bg-[#0f1d35] text-white border border-[#1a2942]'
                      : 'bg-white text-[#333] border border-[#ddd] shadow-sm'
                }`}
              >
                <div className="whitespace-pre-wrap">{message.content}</div>
                <div
                  className={`text-xs mt-1 ${
                    message.role === 'user' 
                      ? theme === 'dark' ? 'text-[#0a1628]/70' : 'text-white/70'
                      : theme === 'dark' ? 'text-gray-400' : 'text-[#999]'
                  }`}
                >
                  {formatTime(message.created_at)}
                </div>
              </div>
            </div>
          ))}
        </div>

        {/* Input Area */}
        <div className={`border-t p-4 ${
          theme === 'dark'
            ? 'border-[#1a2942] bg-[#0f1d35]'
            : 'border-[#ddd] bg-white'
        }`}>
          <div className="flex gap-2">
            <button className={`p-3 rounded-lg transition-colors ${
              theme === 'dark'
                ? 'hover:bg-[#0a1628] text-gray-400'
                : 'hover:bg-[#f5f5f5] text-[#666]'
            }`}>
              <Image size={20} />
            </button>
            <textarea
              value={inputText}
              onChange={(e) => setInputText(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && !e.shiftKey) {
                  e.preventDefault();
                  handleSend();
                }
              }}
              placeholder="输入消息... (Shift+Enter 换行)"
              className={`flex-1 px-4 py-3 border rounded-lg focus:outline-none resize-none ${
                theme === 'dark'
                  ? 'bg-[#0a1628] border-[#1a2942] focus:border-[#5ddad5] text-white placeholder-gray-500'
                  : 'bg-white border-[#ddd] focus:border-[#667eea] focus:ring-2 focus:ring-[#667eea]/20 text-[#333] placeholder-gray-400'
              }`}
              rows={3}
            />
            <button
              onClick={handleSend}
              disabled={loading || !inputText.trim()}
              className={`px-6 rounded-lg font-medium transition-opacity flex items-center gap-2 ${
                theme === 'dark'
                  ? 'bg-gradient-to-r from-[#5ddad5] to-[#4facf7] text-[#0a1628] hover:opacity-90 disabled:opacity-50'
                  : 'bg-[#667eea] text-white hover:opacity-90 shadow-md disabled:opacity-50'
              }`}
            >
              <Send size={20} />
              <span>{loading ? '发送中...' : '发送'}</span>
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
