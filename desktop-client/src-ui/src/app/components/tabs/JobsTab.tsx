import { useState, useEffect } from 'react';
import { Briefcase, Clock, CheckCircle, XCircle, AlertCircle } from 'lucide-react';
import { useTheme } from '../../contexts/ThemeContext';
import { jobApi, JobInfo } from '../../utils/tauri';

export function JobsTab() {
  const { theme } = useTheme();
  const [jobs, setJobs] = useState<JobInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const fetchJobs = async () => {
      try {
        setLoading(true);
        const jobList = await jobApi.getJobs();
        setJobs(jobList);
        setError(null);
      } catch (err) {
        console.error('Failed to fetch jobs:', err);
        setError('Failed to load jobs');
        setJobs([]);
      } finally {
        setLoading(false);
      }
    };

    fetchJobs();
  }, []);

  const getStatusBadge = (status: string) => {
    switch (status) {
      case 'pending':
        return (
          <span className="px-3 py-1 bg-blue-400/10 text-blue-400 text-sm rounded-full border border-blue-400/30 flex items-center gap-2">
            <Clock size={14} />
            待处理
          </span>
        );
      case 'in_progress':
        return (
          <span className="px-3 py-1 bg-yellow-400/10 text-yellow-400 text-sm rounded-full border border-yellow-400/30 flex items-center gap-2">
            <AlertCircle size={14} />
            进行中
          </span>
        );
      case 'completed':
        return (
          <span className="px-3 py-1 bg-green-400/10 text-green-400 text-sm rounded-full border border-green-400/30 flex items-center gap-2">
            <CheckCircle size={14} />
            已完成
          </span>
        );
      case 'failed':
        return (
          <span className="px-3 py-1 bg-red-400/10 text-red-400 text-sm rounded-full border border-red-400/30 flex items-center gap-2">
            <XCircle size={14} />
            失败
          </span>
        );
      case 'stuck':
        return (
          <span className="px-3 py-1 bg-orange-400/10 text-orange-400 text-sm rounded-full border border-orange-400/30 flex items-center gap-2">
            <AlertCircle size={14} />
            卡住
          </span>
        );
      default:
        return null;
    }
  };

  const statusCounts = {
    total: jobs.length,
    pending: jobs.filter(j => j.status === 'pending').length,
    in_progress: jobs.filter(j => j.status === 'in_progress').length,
    completed: jobs.filter(j => j.status === 'completed').length,
    failed: jobs.filter(j => j.status === 'failed').length,
    stuck: jobs.filter(j => j.status === 'stuck').length,
  };

  if (loading) {
    return (
      <div className="h-full flex items-center justify-center">
        <div className={`text-center ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>
          <div className="animate-spin mb-4">
            <Briefcase size={48} className="mx-auto opacity-50" />
          </div>
          <p>加载任务中...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col">
      <div className="p-6">
        <div className="mb-6">
          <h2 className={`text-2xl font-bold mb-4 ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>任务管理</h2>
          {error && (
            <div className="mb-4 p-3 bg-red-400/10 border border-red-400/30 rounded-lg text-red-400 text-sm">
              {error}
            </div>
          )}
          
          <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-4 mb-6">
            <div className={`border rounded-lg p-4 ${
              theme === 'dark'
                ? 'bg-[#0f1d35] border-[#1a2942]'
                : 'bg-white border-[#ddd] shadow-sm'
            }`}>
              <div className={`text-2xl font-bold ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>{statusCounts.total}</div>
              <div className={`text-sm ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>总任务数</div>
            </div>
            <div className={`border rounded-lg p-4 ${
              theme === 'dark'
                ? 'bg-[#0f1d35] border-[#1a2942]'
                : 'bg-white border-[#ddd] shadow-sm'
            }`}>
              <div className="text-2xl font-bold text-blue-400">{statusCounts.pending}</div>
              <div className={`text-sm ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>待处理</div>
            </div>
            <div className={`border rounded-lg p-4 ${
              theme === 'dark'
                ? 'bg-[#0f1d35] border-[#1a2942]'
                : 'bg-white border-[#ddd] shadow-sm'
            }`}>
              <div className="text-2xl font-bold text-yellow-400">{statusCounts.in_progress}</div>
              <div className={`text-sm ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>进行中</div>
            </div>
            <div className={`border rounded-lg p-4 ${
              theme === 'dark'
                ? 'bg-[#0f1d35] border-[#1a2942]'
                : 'bg-white border-[#ddd] shadow-sm'
            }`}>
              <div className="text-2xl font-bold text-green-400">{statusCounts.completed}</div>
              <div className={`text-sm ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>已完成</div>
            </div>
            <div className={`border rounded-lg p-4 ${
              theme === 'dark'
                ? 'bg-[#0f1d35] border-[#1a2942]'
                : 'bg-white border-[#ddd] shadow-sm'
            }`}>
              <div className="text-2xl font-bold text-red-400">{statusCounts.failed}</div>
              <div className={`text-sm ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>失败</div>
            </div>
            <div className={`border rounded-lg p-4 ${
              theme === 'dark'
                ? 'bg-[#0f1d35] border-[#1a2942]'
                : 'bg-white border-[#ddd] shadow-sm'
            }`}>
              <div className="text-2xl font-bold text-orange-400">{statusCounts.stuck}</div>
              <div className={`text-sm ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>卡住</div>
            </div>
          </div>
        </div>

        <div className="space-y-3">
          {jobs.map((job) => (
            <div
              key={job.id}
              className={`border rounded-xl p-5 transition-colors ${
                theme === 'dark'
                  ? 'bg-[#0f1d35] border-[#1a2942] hover:border-[#5ddad5]/30'
                  : 'bg-white border-[#ddd] hover:border-[#667eea]/50 shadow-sm hover:shadow-md'
              }`}
            >
              <div className="flex items-start justify-between mb-3">
                <div className="flex items-start gap-3 flex-1">
                  <Briefcase className={theme === 'dark' ? 'text-[#5ddad5]' : 'text-[#667eea]'} size={20} />
                  <div className="flex-1">
                    <h3 className={`font-semibold text-lg mb-1 ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>{job.title || job.id}</h3>
                    <p className={`text-sm ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>任务ID: {job.id}</p>
                  </div>
                </div>
                {getStatusBadge(job.status)}
              </div>

              <div className="grid grid-cols-2 gap-4 mt-4">
                <div>
                  <div className={`text-xs mb-1 ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>创建时间</div>
                  <div className={`text-sm ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>{new Date(job.created_at).toLocaleString('zh-CN')}</div>
                </div>
                <div>
                  <div className={`text-xs mb-1 ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>更新时间</div>
                  <div className={`text-sm ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>{new Date(job.updated_at).toLocaleString('zh-CN')}</div>
                </div>
              </div>
            </div>
          ))}
        </div>

        {jobs.length === 0 && (
          <div className={`text-center py-12 ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>
            <Briefcase size={48} className="mx-auto mb-4 opacity-50" />
            <p>暂无任务</p>
          </div>
        )}
      </div>
    </div>
  );
}
