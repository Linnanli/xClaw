import { useState, useEffect } from 'react';
import { Plus, ChevronLeft, ChevronRight, Image, Send, Wifi, WifiOff } from 'lucide-react';
import { useTheme } from '../../contexts/ThemeContext';
import { threadApi, type Thread, type Message } from '../../utils/tauri';
import { useAiChat } from '../../hooks/useAiChat';

export function ChatTabWithAiSdk() {
  const { theme } = useTheme();
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [selectedConversation, setSelectedConversation] = useState<string | null>(null);
  const [conversations, setConversations] = useState<Thread[]>([]);
  const [loading, setLoading] = useState(false);
  const [connected, setConnected] = useState(false);

  // 使用 Vercel AI SDK 的 useAiChat Hook
  const chat = useAiChat({
    threadId: selectedConversation || '',
    apiUrl: 'http://localhost:3000',
    authToken: 'd397b61ad5584603d5691f03e73a0a3eea6fd66d1590ddc64a6b0292c7a2f270',
  });

  useEffect(() => {
    loadConversations();
  }, []);

  useEffect(() => {
    if (selectedConversation) {
      loadMessages(selectedConversation);
      setConnected(true);
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
      // 将消息转换为 AI SDK 格式
      const aiMessages = msgs.map(msg => ({
        id: msg.id,
        role: msg.role as 'user' | 'assistant',
        content: msg.content,
      }));
      // 注意: useAiChat 已经管理消息，这里只是初始化
    } catch (err) {
      console.error('Failed to load messages:', err);
    }
  };

  const handleCreateNew = async () => {
    try {
      const newThread = await threadApi.createThread();
      setConversations([newThread, ...conversations]);
      setSelectedConversation(newThread.id);
    } catch (err) {
      console.error('Failed to create new thread:', err);
    }
  };

  const handleSendMessage = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!chat.input.trim() || !selectedConversation) return;

    try {
      setLoading(true);
      // 使用 Hook 的 append 方法发送消息
      await chat.append({
        role: 'user',
        content: chat.input,
      });
      chat.setInput('');
    } catch (err) {
      console.error('Failed to send message:', err);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className={`flex h-full ${theme === 'dark' ? 'bg-gray-900' : 'bg-white'}`}>
      {/* 侧边栏 */}
      <div
        className={`${
          sidebarOpen ? 'w-64' : 'w-0'
        } transition-all duration-300 ${
          theme === 'dark' ? 'bg-gray-800 border-gray-700' : 'bg-gray-50 border-gray-200'
        } border-r overflow-hidden flex flex-col`}
      >
        <div className="p-4 border-b">
          <button
            onClick={handleCreateNew}
            className={`w-full flex items-center justify-center gap-2 px-4 py-2 rounded-lg ${
              theme === 'dark'
                ? 'bg-blue-600 hover:bg-blue-700 text-white'
                : 'bg-blue-500 hover:bg-blue-600 text-white'
            }`}
          >
            <Plus size={20} />
            新对话
          </button>
        </div>

        <div className="flex-1 overflow-y-auto">
          {conversations.map(conv => (
            <button
              key={conv.id}
              onClick={() => setSelectedConversation(conv.id)}
              className={`w-full text-left px-4 py-3 border-b ${
                selectedConversation === conv.id
                  ? theme === 'dark'
                    ? 'bg-gray-700 text-white'
                    : 'bg-gray-200 text-black'
                  : theme === 'dark'
                  ? 'text-gray-300 hover:bg-gray-700'
                  : 'text-gray-700 hover:bg-gray-100'
              }`}
            >
              <div className="truncate font-medium">{conv.title || '新对话'}</div>
              <div className={`text-sm truncate ${theme === 'dark' ? 'text-gray-400' : 'text-gray-500'}`}>
                {new Date(conv.created_at).toLocaleDateString()}
              </div>
            </button>
          ))}
        </div>
      </div>

      {/* 主聊天区域 */}
      <div className="flex-1 flex flex-col">
        {/* 顶部栏 */}
        <div
          className={`flex items-center justify-between px-6 py-4 border-b ${
            theme === 'dark' ? 'bg-gray-800 border-gray-700' : 'bg-gray-50 border-gray-200'
          }`}
        >
          <button
            onClick={() => setSidebarOpen(!sidebarOpen)}
            className={`p-2 rounded-lg ${
              theme === 'dark' ? 'hover:bg-gray-700' : 'hover:bg-gray-200'
            }`}
          >
            {sidebarOpen ? <ChevronLeft size={20} /> : <ChevronRight size={20} />}
          </button>

          <div className="flex items-center gap-2">
            {connected ? (
              <div className="flex items-center gap-2 text-green-500">
                <Wifi size={16} />
                <span className="text-sm">已连接</span>
              </div>
            ) : (
              <div className="flex items-center gap-2 text-red-500">
                <WifiOff size={16} />
                <span className="text-sm">未连接</span>
              </div>
            )}
          </div>
        </div>

        {/* 消息区域 */}
        <div className="flex-1 overflow-y-auto p-6 space-y-4">
          {/* 错误提示 */}
          {chat.error && (
            <div className={`p-4 rounded-lg border ${
              theme === 'dark'
                ? 'bg-red-900 border-red-700 text-red-100'
                : 'bg-red-100 border-red-300 text-red-800'
            }`}>
              <div className="font-semibold">❌ 错误</div>
              <div className="text-sm mt-1">{chat.error.message}</div>
              <div className={`text-xs mt-2 ${theme === 'dark' ? 'text-red-200' : 'text-red-700'}`}>
                <p>可能的原因：</p>
                <ul className="list-disc list-inside mt-1">
                  <li>LLM API 密钥未配置或无效</li>
                  <li>后端服务未运行</li>
                  <li>网络连接问题</li>
                </ul>
              </div>
            </div>
          )}
          
          {chat.messages.length === 0 ? (
            <div className={`text-center py-12 ${theme === 'dark' ? 'text-gray-400' : 'text-gray-500'}`}>
              <p>开始一个新的对话</p>
            </div>
          ) : (
            chat.messages.map(msg => (
              <div
                key={msg.id}
                className={`flex ${msg.role === 'user' ? 'justify-end' : 'justify-start'}`}
              >
                <div
                  className={`max-w-xs lg:max-w-md px-4 py-2 rounded-lg ${
                    msg.role === 'user'
                      ? theme === 'dark'
                        ? 'bg-blue-600 text-white'
                        : 'bg-blue-500 text-white'
                      : theme === 'dark'
                      ? 'bg-gray-700 text-gray-100'
                      : 'bg-gray-200 text-gray-900'
                  }`}
                >
                  <p className="text-sm">{msg.content}</p>
                </div>
              </div>
            ))
          )}
          {chat.isLoading && (
            <div className="flex justify-start">
              <div className={`px-4 py-2 rounded-lg ${theme === 'dark' ? 'bg-gray-700' : 'bg-gray-200'}`}>
                <div className="flex gap-2">
                  <div className="w-2 h-2 bg-gray-500 rounded-full animate-bounce"></div>
                  <div className="w-2 h-2 bg-gray-500 rounded-full animate-bounce" style={{ animationDelay: '0.1s' }}></div>
                  <div className="w-2 h-2 bg-gray-500 rounded-full animate-bounce" style={{ animationDelay: '0.2s' }}></div>
                </div>
              </div>
            </div>
          )}
        </div>

        {/* 输入区域 */}
        <div className={`border-t p-4 ${theme === 'dark' ? 'bg-gray-800 border-gray-700' : 'bg-gray-50 border-gray-200'}`}>
          <form onSubmit={handleSendMessage} className="flex gap-2">
            <button
              type="button"
              className={`p-2 rounded-lg ${
                theme === 'dark' ? 'hover:bg-gray-700' : 'hover:bg-gray-200'
              }`}
            >
              <Image size={20} />
            </button>
            <input
              type="text"
              value={chat.input}
              onChange={e => chat.setInput(e.target.value)}
              placeholder="输入消息..."
              className={`flex-1 px-4 py-2 rounded-lg border ${
                theme === 'dark'
                  ? 'bg-gray-700 border-gray-600 text-white placeholder-gray-400'
                  : 'bg-white border-gray-300 text-black placeholder-gray-500'
              }`}
              disabled={loading || !selectedConversation}
            />
            <button
              type="submit"
              disabled={loading || !chat.input.trim() || !selectedConversation}
              className={`p-2 rounded-lg ${
                loading || !chat.input.trim() || !selectedConversation
                  ? theme === 'dark'
                    ? 'bg-gray-700 text-gray-500'
                    : 'bg-gray-300 text-gray-500'
                  : theme === 'dark'
                  ? 'bg-blue-600 hover:bg-blue-700 text-white'
                  : 'bg-blue-500 hover:bg-blue-600 text-white'
              }`}
            >
              <Send size={20} />
            </button>
          </form>
        </div>
      </div>
    </div>
  );
}
