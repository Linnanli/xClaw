import { useState, useEffect } from 'react';
import { Plus, Play, Trash2, Pause, PlayCircle } from 'lucide-react';
import { useTheme } from '../../contexts/ThemeContext';
import { routineApi, type Routine } from '../../utils/tauri';

export function RoutinesTab() {
  const { theme } = useTheme();
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [routines, setRoutines] = useState<Routine[]>([]);
  const [loading, setLoading] = useState(false);

  const [newRoutine, setNewRoutine] = useState({
    name: '',
    description: '',
    trigger: 'manual' as const,
    triggerValue: '',
  });

  useEffect(() => {
    loadRoutines();
  }, []);

  const loadRoutines = async () => {
    setLoading(true);
    try {
      const data = await routineApi.getRoutines();
      setRoutines(data);
    } catch (err) {
      console.error('Failed to load routines:', err);
    } finally {
      setLoading(false);
    }
  };

  const handleCreate = async () => {
    if (newRoutine.name && newRoutine.description) {
      try {
        let trigger;
        if (newRoutine.trigger === 'manual') {
          trigger = { Manual: null };
        } else if (newRoutine.trigger === 'time') {
          trigger = { Time: newRoutine.triggerValue || '0 9 * * *' };
        } else {
          trigger = { Event: newRoutine.triggerValue || 'default_event' };
        }

        await routineApi.createRoutine(
          newRoutine.name,
          newRoutine.description,
          trigger,
          []
        );
        
        setNewRoutine({ name: '', description: '', trigger: 'manual', triggerValue: '' });
        setShowCreateModal(false);
        await loadRoutines();
      } catch (err) {
        console.error('Failed to create routine:', err);
      }
    }
  };

  const handleDelete = async (id: string) => {
    if (confirm('确定要删除此日程吗？')) {
      try {
        await routineApi.deleteRoutine(id);
        await loadRoutines();
      } catch (err) {
        console.error('Failed to delete routine:', err);
      }
    }
  };

  const handleTrigger = async (id: string) => {
    try {
      await routineApi.triggerRoutine(id);
    } catch (err) {
      console.error('Failed to trigger routine:', err);
    }
  };

  const getStatusBadge = (status: string) => {
    switch (status) {
      case 'active':
        return <span className="px-3 py-1 bg-green-400/10 text-green-400 text-sm rounded-full border border-green-400/30">活跃</span>;
      case 'paused':
        return <span className="px-3 py-1 bg-yellow-400/10 text-yellow-400 text-sm rounded-full border border-yellow-400/30">暂停</span>;
      case 'disabled':
        return <span className="px-3 py-1 bg-gray-400/10 text-gray-400 text-sm rounded-full border border-gray-400/30">禁用</span>;
      default:
        return null;
    }
  };

  const getTriggerLabel = (trigger: string) => {
    switch (trigger) {
      case 'manual':
        return '手动';
      case 'time':
        return '时间';
      case 'event':
        return '事件';
      default:
        return trigger;
    }
  };

  return (
    <div className="h-full flex flex-col">
      <div className="p-6">
        <div className="flex justify-between items-center mb-6">
          <h2 className={`text-2xl font-bold ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>日程管理</h2>
          <button
            onClick={() => setShowCreateModal(true)}
            className={`flex items-center gap-2 px-4 py-2 rounded-lg font-medium transition-opacity ${
              theme === 'dark'
                ? 'bg-gradient-to-r from-[#5ddad5] to-[#4facf7] text-[#0a1628] hover:opacity-90'
                : 'bg-[#667eea] text-white hover:opacity-90 shadow-md'
            }`}
          >
            <Plus size={18} />
            创建日程
          </button>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {routines.map((routine) => (
            <div
              key={routine.id}
              className={`border rounded-xl p-5 transition-colors ${
                theme === 'dark'
                  ? 'bg-[#0f1d35] border-[#1a2942] hover:border-[#5ddad5]/30'
                  : 'bg-white border-[#ddd] hover:border-[#667eea]/50 shadow-sm hover:shadow-md'
              }`}
            >
              <div className="flex items-start justify-between mb-3">
                <h3 className={`font-semibold text-lg ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>{routine.name}</h3>
                {getStatusBadge(routine.status)}
              </div>

              <p className={`text-sm mb-3 ${theme === 'dark' ? 'text-gray-400' : 'text-[#666]'}`}>{routine.description}</p>

              <div className={`text-sm mb-3 ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>
                <span className="font-medium">触发器:</span> {getTriggerLabel(routine.trigger)}
                {routine.triggerValue && (
                  <span className="ml-2">({routine.triggerValue})</span>
                )}
              </div>

              <div className="flex gap-2">
                <button
                  onClick={() => handleTrigger(routine.id)}
                  className={`flex-1 px-3 py-2 rounded-lg flex items-center justify-center gap-2 transition-opacity ${
                    theme === 'dark'
                      ? 'bg-gradient-to-r from-[#5ddad5] to-[#4facf7] text-[#0a1628] hover:opacity-90'
                      : 'bg-[#667eea] text-white hover:opacity-90 shadow-sm'
                  }`}
                >
                  <PlayCircle size={16} />
                  触发
                </button>
                <button
                  onClick={() => handleTogglePause(routine.id)}
                  className={`px-3 py-2 rounded-lg transition-opacity border ${
                    theme === 'dark'
                      ? 'bg-[#0a1628] hover:opacity-80 text-gray-400 border-[#1a2942]'
                      : 'bg-[#f5f5f5] hover:opacity-80 text-[#666] border-[#ddd]'
                  }`}
                >
                  {routine.status === 'active' ? <Pause size={16} /> : <Play size={16} />}
                </button>
                <button
                  onClick={() => handleDelete(routine.id)}
                  className="px-3 py-2 bg-red-400/10 hover:bg-red-400/20 text-red-400 rounded-lg transition-colors border border-red-400/30"
                >
                  <Trash2 size={16} />
                </button>
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Create Modal */}
      {showCreateModal && (
        <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
          <div className={`border rounded-xl p-6 max-w-md w-full mx-4 ${
            theme === 'dark'
              ? 'bg-[#0f1d35] border-[#1a2942]'
              : 'bg-white border-[#ddd] shadow-lg'
          }`}>
            <h3 className={`text-lg font-semibold mb-4 ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>
              创建新日程
            </h3>

            <div className="space-y-4">
              <div>
                <label className={`block text-sm font-medium mb-2 ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>
                  日程名称
                </label>
                <input
                  type="text"
                  value={newRoutine.name}
                  onChange={(e) => setNewRoutine({ ...newRoutine, name: e.target.value })}
                  className={`w-full px-4 py-2 border rounded-lg focus:outline-none ${
                    theme === 'dark'
                      ? 'bg-[#0a1628] border-[#1a2942] focus:border-[#5ddad5] text-white placeholder-gray-500'
                      : 'bg-white border-[#ddd] focus:border-[#667eea] focus:ring-2 focus:ring-[#667eea]/20 text-[#333] placeholder-gray-400'
                  }`}
                  placeholder="输入日程名称"
                />
              </div>

              <div>
                <label className={`block text-sm font-medium mb-2 ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>
                  描述
                </label>
                <textarea
                  value={newRoutine.description}
                  onChange={(e) => setNewRoutine({ ...newRoutine, description: e.target.value })}
                  className={`w-full px-4 py-2 border rounded-lg focus:outline-none resize-none ${
                    theme === 'dark'
                      ? 'bg-[#0a1628] border-[#1a2942] focus:border-[#5ddad5] text-white placeholder-gray-500'
                      : 'bg-white border-[#ddd] focus:border-[#667eea] focus:ring-2 focus:ring-[#667eea]/20 text-[#333] placeholder-gray-400'
                  }`}
                  rows={3}
                  placeholder="输入日程描述"
                />
              </div>

              <div>
                <label className={`block text-sm font-medium mb-2 ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>
                  触发器类型
                </label>
                <select
                  value={newRoutine.trigger}
                  onChange={(e) => setNewRoutine({ ...newRoutine, trigger: e.target.value as 'manual' | 'time' | 'event' })}
                  className={`w-full px-4 py-2 border rounded-lg focus:outline-none ${
                    theme === 'dark'
                      ? 'bg-[#0a1628] border-[#1a2942] focus:border-[#5ddad5] text-white'
                      : 'bg-white border-[#ddd] focus:border-[#667eea] focus:ring-2 focus:ring-[#667eea]/20 text-[#333]'
                  }`}
                >
                  <option value="manual">手动</option>
                  <option value="time">时间</option>
                  <option value="event">事件</option>
                </select>
              </div>

              {newRoutine.trigger === 'time' && (
                <div>
                  <label className={`block text-sm font-medium mb-2 ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>
                    Cron表达式
                  </label>
                  <input
                    type="text"
                    value={newRoutine.triggerValue}
                    onChange={(e) => setNewRoutine({ ...newRoutine, triggerValue: e.target.value })}
                    className={`w-full px-4 py-2 border rounded-lg focus:outline-none ${
                      theme === 'dark'
                        ? 'bg-[#0a1628] border-[#1a2942] focus:border-[#5ddad5] text-white placeholder-gray-500'
                        : 'bg-white border-[#ddd] focus:border-[#667eea] focus:ring-2 focus:ring-[#667eea]/20 text-[#333] placeholder-gray-400'
                    }`}
                    placeholder="例如: 0 9 * * *"
                  />
                </div>
              )}
            </div>

            <div className="flex gap-3 justify-end mt-6">
              <button
                onClick={() => setShowCreateModal(false)}
                className={`px-4 py-2 rounded-lg transition-opacity border ${
                  theme === 'dark'
                    ? 'bg-[#0a1628] hover:opacity-80 text-gray-400 border-[#1a2942]'
                    : 'bg-[#f5f5f5] hover:opacity-80 text-[#666] border-[#ddd]'
                }`}
              >
                取消
              </button>
              <button
                onClick={handleCreate}
                className={`px-4 py-2 rounded-lg font-medium transition-opacity ${
                  theme === 'dark'
                    ? 'bg-gradient-to-r from-[#5ddad5] to-[#4facf7] text-[#0a1628] hover:opacity-90'
                    : 'bg-[#667eea] text-white hover:opacity-90 shadow-md'
                }`}
              >
                创建
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
