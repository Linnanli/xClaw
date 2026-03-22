/**
 * Chat Tab - Tauri IPC 版本
 * 
 * 使用 Tauri IPC 通信替代 HTTP API
 * 
 * 主要改进:
 * - 使用 useAiChatTauri Hook (Tauri IPC)
 * - 更快的响应速度（无网络开销）
 * - 更安全（无需暴露 HTTP 端口）
 * - 更好的类型安全
 */

import { useState, useEffect } from 'react';
import { Plus, ChevronLeft, ChevronRight, Send, Wifi, WifiOff } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { useTheme } from '../../contexts/ThemeContext';
import { threadApi, type Thread } from '../../utils/tauri';
import { useAiChatTauri } from '../../hooks/useAiChatTauri';
import { TokenManager } from '../../utils/tokenManager';
import { MessageActions } from '../common/MessageActions';
import { MessageEditor } from '../common/MessageEditor';
import { DeleteConfirmDialog } from '../common/DeleteConfirmDialog';
import { DlpStatusIndicator } from '../DlpStatusIndicator';

export function ChatTabTauri() {
  const { theme } = useTheme();
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [selectedConversation, setSelectedConversation] = useState<string | null>(null);
  const [conversations, setConversations] = useState<Thread[]>([]);
  const [loading, setLoading] = useState(false);
  const [authToken, setAuthToken] = useState<string>('');
  
  // 消息编辑/删除状态
  const [editingMessageId, setEditingMessageId] = useState<string | null>(null);
  const [deleteConfirmDialog, setDeleteConfirmDialog] = useState<{
    isOpen: boolean;
    messageId: string | null;
    content: string;
  }>({
    isOpen: false,
    messageId: null,
    content: '',
  });
  const [operationLoading, setOperationLoading] = useState(false);

  // 获取认证令牌
  useEffect(() => {
    const loadToken = async () => {
      try {
        const token = await TokenManager.getToken();
        setAuthToken(token);
        console.log('✅ Token loaded');
      } catch (err) {
        console.error('❌ Failed to load token:', err);
      }
    };
    loadToken();

    // 启动时同步 DLP 规则
    invoke('sync_dlp_rules_from_admin')
      .then((result: unknown) => {
        console.log('✅ DLP rules synced:', result);
      })
      .catch((err: unknown) => {
        console.warn('⚠️ DLP rules sync failed (admin backend may be offline):', err);
      });
  }, []);

  // 使用 Tauri IPC 的 useAiChatTauri Hook
  const chat = useAiChatTauri({
    threadId: selectedConversation || '',
    onError: (error) => {
      console.error('Chat error:', error);
    },
    onStatusChange: (status) => {
      console.log('Chat status:', status);
    },
  });

  // 加载对话列表
  useEffect(() => {
    if (authToken) {
      loadConversations();
    }
  }, [authToken]);

  // 选择对话时加载消息
  useEffect(() => {
    if (selectedConversation) {
      loadMessages(selectedConversation);
    }
  }, [selectedConversation]);

  const loadConversations = async () => {
    try {
      const response = await threadApi.getThreads();
      const threads = Array.isArray(response) ? response : (response.threads || []);
      setConversations(threads);
      
      if (threads.length === 0) {
        const newThread = await threadApi.createThread();
        setConversations([newThread]);
        setSelectedConversation(newThread.id);
      } else if (!selectedConversation) {
        setSelectedConversation(threads[0].id);
      }
    } catch (err) {
      console.error('Failed to load conversations:', err);
    }
  };

  const loadMessages = async (threadId: string) => {
    try {
      setLoading(true);
      const messages = await threadApi.getMessages(threadId);
      chat.setMessages(messages.map(m => ({
        id: m.id,
        role: m.role as 'user' | 'assistant',
        content: m.content,
        timestamp: new Date(m.created_at).getTime(),
      })));
    } catch (err) {
      console.error('Failed to load messages:', err);
    } finally {
      setLoading(false);
    }
  };

  const handleNewConversation = async () => {
    try {
      const newThread = await threadApi.createThread();
      setConversations(prev => [newThread, ...prev]);
      setSelectedConversation(newThread.id);
      chat.clearMessages();
    } catch (err) {
      console.error('Failed to create conversation:', err);
    }
  };

  const handleDeleteMessage = async (messageId: string) => {
    setOperationLoading(true);
    try {
      // TODO: 实现删除消息
      chat.setMessages(prev => prev.filter(m => m.id !== messageId));
      setDeleteConfirmDialog({ isOpen: false, messageId: null, content: '' });
    } catch (err) {
      console.error('Failed to delete message:', err);
    } finally {
      setOperationLoading(false);
    }
  };

  const handleSaveMessage = async (messageId: string, newContent: string) => {
    setOperationLoading(true);
    try {
      // TODO: 实现保存消息
      chat.setMessages(prev =>
        prev.map(m => (m.id === messageId ? { ...m, content: newContent } : m))
      );
      setEditingMessageId(null);
    } catch (err) {
      console.error('Failed to save message:', err);
    } finally {
      setOperationLoading(false);
    }
  };

  return (
    <div className="flex h-full">
      {/* 侧边栏 */}
      <div
        className={`${
          sidebarOpen ? 'w-64' : 'w-0'
        } transition-all duration-300 border-r ${
          theme === 'dark' ? 'border-gray-700 bg-gray-800' : 'border-gray-200 bg-white'
        } overflow-hidden`}
      >
        <div className="p-4">
          <button
            onClick={handleNewConversation}
            className={`w-full flex items-center gap-2 px-4 py-2 rounded-lg ${
              theme === 'dark'
                ? 'bg-blue-600 hover:bg-blue-700 text-white'
                : 'bg-blue-500 hover:bg-blue-600 text-white'
            }`}
          >
            <Plus size={20} />
            <span>新对话</span>
          </button>

          <div className="mt-4 space-y-2">
            {conversations.map((conv) => (
              <button
                key={conv.id}
                onClick={() => setSelectedConversation(conv.id)}
                className={`w-full text-left px-4 py-2 rounded-lg transition-colors ${
                  selectedConversation === conv.id
                    ? theme === 'dark'
                      ? 'bg-gray-700 text-white'
                      : 'bg-gray-100 text-gray-900'
                    : theme === 'dark'
                    ? 'hover:bg-gray-700 text-gray-300'
                    : 'hover:bg-gray-50 text-gray-700'
                }`}
              >
                <div className="truncate">{conv.title || '新对话'}</div>
              </button>
            ))}
          </div>
        </div>
      </div>

      {/* 主聊天区域 */}
      <div className="flex-1 flex flex-col">
        {/* 顶部栏 */}
        <div
          className={`flex items-center justify-between px-4 py-3 border-b ${
            theme === 'dark' ? 'border-gray-700 bg-gray-800' : 'border-gray-200 bg-white'
          }`}
        >
          <div className="flex items-center gap-2">
            <button
              onClick={() => setSidebarOpen(!sidebarOpen)}
              className={`p-2 rounded-lg ${
                theme === 'dark' ? 'hover:bg-gray-700' : 'hover:bg-gray-100'
              }`}
            >
              {sidebarOpen ? <ChevronLeft size={20} /> : <ChevronRight size={20} />}
            </button>
            <h2 className="text-lg font-semibold">
              {conversations.find((c) => c.id === selectedConversation)?.title || '新对话'}
            </h2>
          </div>

          <div className="flex items-center gap-3">
            {/* DLP 状态指示器 */}
            <DlpStatusIndicator />

            {/* 连接状态 */}
            <div className="flex items-center gap-2">
              {chat.isConnected ? (
                <>
                  <Wifi size={16} className="text-green-500" />
                  <span className="text-sm text-green-500">已连接</span>
                </>
              ) : (
                <div className="flex items-center gap-2 cursor-pointer" title="点击重试连接"
                  onClick={() => {
                    // 重新订阅
                    invoke('subscribe_chat_events').catch(console.error);
                  }}
                >
                  <WifiOff size={16} className="text-red-500" />
                  <span className="text-sm text-red-500">未连接</span>
                </div>
              )}
            </div>
          </div>
        </div>

        {/* 消息列表 */}
        <div className="flex-1 overflow-y-auto p-4 space-y-4">
          {loading ? (
            <div className="text-center text-gray-500">加载中...</div>
          ) : chat.messages.length === 0 ? (
            <div className="text-center text-gray-500">开始新对话</div>
          ) : (
            chat.messages.map((message) => (
              <div
                key={message.id}
                className={`flex ${message.role === 'user' ? 'justify-end' : 'justify-start'}`}
              >
                <div
                  className={`max-w-[70%] rounded-lg p-4 ${
                    message.role === 'user'
                      ? theme === 'dark'
                        ? 'bg-blue-600 text-white'
                        : 'bg-blue-500 text-white'
                      : theme === 'dark'
                      ? 'bg-gray-700 text-gray-100'
                      : 'bg-gray-100 text-gray-900'
                  }`}
                >
                  {editingMessageId === message.id ? (
                    <MessageEditor
                      messageId={message.id}
                      initialContent={message.content}
                      onSave={handleSaveMessage}
                      onCancel={() => setEditingMessageId(null)}
                      disabled={operationLoading}
                    />
                  ) : (
                    <>
                      <div className="whitespace-pre-wrap">{message.content}</div>
                      {message.role === 'user' && (
                        <MessageActions
                          messageId={message.id}
                          content={message.content}
                          onEdit={() => setEditingMessageId(message.id)}
                          onDelete={() =>
                            setDeleteConfirmDialog({
                              isOpen: true,
                              messageId: message.id,
                              content: message.content,
                            })
                          }
                        />
                      )}
                    </>
                  )}
                </div>
              </div>
            ))
          )}

          {/* 思考状态 */}
          {chat.thinkingMessage && (
            <div className="flex justify-start">
              <div
                className={`max-w-[70%] rounded-lg p-4 ${
                  theme === 'dark' ? 'bg-gray-700 text-gray-300' : 'bg-gray-100 text-gray-600'
                }`}
              >
                <div className="flex items-center gap-2">
                  <div className="animate-pulse">💭</div>
                  <span>{chat.thinkingMessage}</span>
                </div>
              </div>
            </div>
          )}

          {/* 错误提示 */}
          {chat.error && (
            <div className="flex justify-center">
              <div className="bg-red-100 text-red-700 px-4 py-2 rounded-lg">
                {chat.error}
              </div>
            </div>
          )}
        </div>

        {/* 输入区域 */}
        <div
          className={`border-t p-4 ${
            theme === 'dark' ? 'border-gray-700 bg-gray-800' : 'border-gray-200 bg-white'
          }`}
        >
          <form onSubmit={chat.handleSubmit} className="flex gap-2">
            <input
              type="text"
              value={chat.input}
              onChange={(e) => chat.setInput(e.target.value)}
              placeholder="输入消息..."
              disabled={chat.isLoading || !chat.isConnected}
              className={`flex-1 px-4 py-2 rounded-lg border ${
                theme === 'dark'
                  ? 'bg-gray-700 border-gray-600 text-white placeholder-gray-400'
                  : 'bg-white border-gray-300 text-gray-900 placeholder-gray-500'
              } focus:outline-none focus:ring-2 focus:ring-blue-500`}
            />
            <button
              type="submit"
              disabled={chat.isLoading || !chat.input.trim() || !chat.isConnected}
              className={`px-6 py-2 rounded-lg flex items-center gap-2 ${
                chat.isLoading || !chat.input.trim() || !chat.isConnected
                  ? 'bg-gray-400 cursor-not-allowed'
                  : theme === 'dark'
                  ? 'bg-blue-600 hover:bg-blue-700'
                  : 'bg-blue-500 hover:bg-blue-600'
              } text-white transition-colors`}
            >
              <Send size={16} />
              <span>{chat.isLoading ? '发送中...' : '发送'}</span>
            </button>
          </form>
        </div>
      </div>

      {/* 删除确认对话框 */}
      <DeleteConfirmDialog
        isOpen={deleteConfirmDialog.isOpen}
        messageContent={deleteConfirmDialog.content}
        onConfirm={() => {
          if (deleteConfirmDialog.messageId) {
            handleDeleteMessage(deleteConfirmDialog.messageId);
          }
        }}
        onCancel={() =>
          setDeleteConfirmDialog({ isOpen: false, messageId: null, content: '' })
        }
        loading={operationLoading}
      />
    </div>
  );
}
