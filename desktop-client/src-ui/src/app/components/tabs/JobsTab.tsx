import { useState, useEffect } from 'react';
import { Briefcase, Clock, CheckCircle, XCircle, AlertCircle, RefreshCw, X, Play, StopCircle } from 'lucide-react';
import { useTheme } from '../../contexts/ThemeContext';
import { jobApi, JobInfo, JobDetail } from '../../utils/tauri';

type StatusFilter = 'all' | 'pending' | 'in_progress' | 'completed' | 'failed' | 'stuck';

export function JobsTab() {
  const { theme } = useTheme();
  const [jobs, setJobs] = useState<JobInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedJob, setSelectedJob] = useState<JobDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [statusFilter, setStatusFilter] = useState<StatusFilter>('all');

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

  useEffect(() => {
    fetchJobs();
  }, []);

  const handleJobClick = async (jobId: string) => {
    try {
      setDetailLoading(true);
      const detail = await jobApi.getJobDetail(jobId);
      setSelectedJob(detail);
    } catch (err) {
      console.error('Failed to fetch job detail:', err);
      setError('Failed to load job detail');
    } finally {
      setDetailLoading(false);
    }
  };

  const handleCancelJob = async (jobId: string) => {
    try {
      await jobApi.cancelJob(jobId);
      setSelectedJob(null);
      await fetchJobs();
    } catch (err) {
      console.error('Failed to cancel job:', err);
      setError('Failed to cancel job');
    }
  };

  const handleRestartJob = async (jobId: string) => {
    try {
      await jobApi.restartJob(jobId);
      setSelectedJob(null);
      await fetchJobs();
    } catch (err) {
      console.error('Failed to restart job:', err);
      setError('Failed to restart job');
    }
  };

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

  const filteredJobs = statusFilter === 'all' 
    ? jobs 
    : jobs.filter(j => j.status === statusFilter);

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
          <div className="flex items-center justify-between mb-4">
            <h2 className={`text-2xl font-bold ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>任务管理</h2>
            <button
              onClick={fetchJobs}
              aria-label="刷新"
              className={`p-2 rounded-lg transition-colors ${
                theme === 'dark'
                  ? 'hover:bg-[#1a2942] text-[#5ddad5]'
                  : 'hover:bg-gray-100 text-[#667eea]'
              }`}
            >
              <RefreshCw size={20} />
            </button>
          </div>

          {error && (
            <div className="mb-4 p-3 bg-red-400/10 border border-red-400/30 rounded-lg text-red-400 text-sm">
              {error}
            </div>
          )}
          
          <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-4 mb-6">
            <button
              onClick={() => setStatusFilter('all')}
              className={`border rounded-lg p-4 text-left transition-colors ${
                statusFilter === 'all'
                  ? theme === 'dark'
                    ? 'bg-[#1a2942] border-[#5ddad5]'
                    : 'bg-[#667eea]/10 border-[#667eea]'
                  : theme === 'dark'
                    ? 'bg-[#0f1d35] border-[#1a2942] hover:border-[#5ddad5]/30'
                    : 'bg-white border-[#ddd] hover:border-[#667eea]/50 shadow-sm'
              }`}
            >
              <div className={`text-2xl font-bold ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>{statusCounts.total}</div>
              <div className={`text-sm ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>总任务数</div>
            </button>
            <button
              onClick={() => setStatusFilter('pending')}
              className={`border rounded-lg p-4 text-left transition-colors ${
                statusFilter === 'pending'
                  ? theme === 'dark'
                    ? 'bg-[#1a2942] border-blue-400'
                    : 'bg-blue-50 border-blue-400'
                  : theme === 'dark'
                    ? 'bg-[#0f1d35] border-[#1a2942] hover:border-blue-400/30'
                    : 'bg-white border-[#ddd] hover:border-blue-400/50 shadow-sm'
              }`}
            >
              <div className="text-2xl font-bold text-blue-400">{statusCounts.pending}</div>
              <div className={`text-sm ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>待处理</div>
            </button>
            <button
              onClick={() => setStatusFilter('in_progress')}
              className={`border rounded-lg p-4 text-left transition-colors ${
                statusFilter === 'in_progress'
                  ? theme === 'dark'
                    ? 'bg-[#1a2942] border-yellow-400'
                    : 'bg-yellow-50 border-yellow-400'
                  : theme === 'dark'
                    ? 'bg-[#0f1d35] border-[#1a2942] hover:border-yellow-400/30'
                    : 'bg-white border-[#ddd] hover:border-yellow-400/50 shadow-sm'
              }`}
            >
              <div className="text-2xl font-bold text-yellow-400">{statusCounts.in_progress}</div>
              <div className={`text-sm ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>进行中</div>
            </button>
            <button
              onClick={() => setStatusFilter('completed')}
              className={`border rounded-lg p-4 text-left transition-colors ${
                statusFilter === 'completed'
                  ? theme === 'dark'
                    ? 'bg-[#1a2942] border-green-400'
                    : 'bg-green-50 border-green-400'
                  : theme === 'dark'
                    ? 'bg-[#0f1d35] border-[#1a2942] hover:border-green-400/30'
                    : 'bg-white border-[#ddd] hover:border-green-400/50 shadow-sm'
              }`}
            >
              <div className="text-2xl font-bold text-green-400">{statusCounts.completed}</div>
              <div className={`text-sm ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>已完成</div>
            </button>
            <button
              onClick={() => setStatusFilter('failed')}
              className={`border rounded-lg p-4 text-left transition-colors ${
                statusFilter === 'failed'
                  ? theme === 'dark'
                    ? 'bg-[#1a2942] border-red-400'
                    : 'bg-red-50 border-red-400'
                  : theme === 'dark'
                    ? 'bg-[#0f1d35] border-[#1a2942] hover:border-red-400/30'
                    : 'bg-white border-[#ddd] hover:border-red-400/50 shadow-sm'
              }`}
            >
              <div className="text-2xl font-bold text-red-400">{statusCounts.failed}</div>
              <div className={`text-sm ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>失败</div>
            </button>
            <button
              onClick={() => setStatusFilter('stuck')}
              className={`border rounded-lg p-4 text-left transition-colors ${
                statusFilter === 'stuck'
                  ? theme === 'dark'
                    ? 'bg-[#1a2942] border-orange-400'
                    : 'bg-orange-50 border-orange-400'
                  : theme === 'dark'
                    ? 'bg-[#0f1d35] border-[#1a2942] hover:border-orange-400/30'
                    : 'bg-white border-[#ddd] hover:border-orange-400/50 shadow-sm'
              }`}
            >
              <div className="text-2xl font-bold text-orange-400">{statusCounts.stuck}</div>
              <div className={`text-sm ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>卡住</div>
            </button>
          </div>
        </div>

        <div className="space-y-3">
          {filteredJobs.map((job) => (
            <button
              key={job.id}
              onClick={() => handleJobClick(job.id)}
              className={`w-full border rounded-xl p-5 transition-colors text-left ${
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
                  <div className={`text-sm ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>
                    {job.updated_at ? new Date(job.updated_at).toLocaleString('zh-CN') : '-'}
                  </div>
                </div>
              </div>
            </button>
          ))}
        </div>

        {filteredJobs.length === 0 && (
          <div className={`text-center py-12 ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>
            <Briefcase size={48} className="mx-auto mb-4 opacity-50" />
            <p>暂无任务</p>
          </div>
        )}
      </div>

      {/* Job Detail Modal */}
      {selectedJob && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
          <div className={`max-w-2xl w-full max-h-[80vh] overflow-y-auto rounded-xl ${
            theme === 'dark' ? 'bg-[#0f1d35]' : 'bg-white'
          }`}>
            <div className={`sticky top-0 p-6 border-b ${
              theme === 'dark' ? 'border-[#1a2942] bg-[#0f1d35]' : 'border-[#ddd] bg-white'
            }`}>
              <div className="flex items-center justify-between">
                <h3 className={`text-xl font-bold ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>任务详情</h3>
                <button
                  onClick={() => setSelectedJob(null)}
                  className={`p-2 rounded-lg transition-colors ${
                    theme === 'dark'
                      ? 'hover:bg-[#1a2942] text-gray-400'
                      : 'hover:bg-gray-100 text-[#999]'
                  }`}
                >
                  <X size={20} />
                </button>
              </div>
            </div>

            <div className="p-6">
              {detailLoading ? (
                <div className="text-center py-8">
                  <div className="animate-spin mb-4">
                    <Briefcase size={48} className="mx-auto opacity-50" />
                  </div>
                  <p className={theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}>加载中...</p>
                </div>
              ) : (
                <>
                  <div className="mb-6">
                    <div className="flex items-center justify-between mb-4">
                      <h4 className={`text-lg font-semibold ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>
                        {selectedJob.title || selectedJob.id}
                      </h4>
                      {getStatusBadge(selectedJob.status)}
                    </div>
                    {selectedJob.description && (
                      <p className={`text-sm ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>
                        {selectedJob.description}
                      </p>
                    )}
                  </div>

                  <div className="grid grid-cols-2 gap-4 mb-6">
                    <div>
                      <div className={`text-xs mb-1 ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>任务ID</div>
                      <div className={`text-sm font-mono ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>{selectedJob.id}</div>
                    </div>
                    <div>
                      <div className={`text-xs mb-1 ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>状态</div>
                      <div className={`text-sm ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>{selectedJob.status}</div>
                    </div>
                    <div>
                      <div className={`text-xs mb-1 ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>创建时间</div>
                      <div className={`text-sm ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>
                        {new Date(selectedJob.created_at).toLocaleString('zh-CN')}
                      </div>
                    </div>
                    <div>
                      <div className={`text-xs mb-1 ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>更新时间</div>
                      <div className={`text-sm ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>
                        {selectedJob.updated_at ? new Date(selectedJob.updated_at).toLocaleString('zh-CN') : '-'}
                      </div>
                    </div>
                  </div>

                  <div className="flex gap-3">
                    {(selectedJob.status === 'pending' || selectedJob.status === 'in_progress') && (
                      <button
                        onClick={() => handleCancelJob(selectedJob.id)}
                        className="flex-1 px-4 py-2 bg-red-500 hover:bg-red-600 text-white rounded-lg transition-colors flex items-center justify-center gap-2"
                      >
                        <StopCircle size={16} />
                        取消任务
                      </button>
                    )}
                    {(selectedJob.status === 'completed' || selectedJob.status === 'failed' || selectedJob.status === 'stuck') && (
                      <button
                        onClick={() => handleRestartJob(selectedJob.id)}
                        className={`flex-1 px-4 py-2 rounded-lg transition-colors flex items-center justify-center gap-2 ${
                          theme === 'dark'
                            ? 'bg-[#5ddad5] hover:bg-[#4bc9bf] text-[#0f1d35]'
                            : 'bg-[#667eea] hover:bg-[#5568d3] text-white'
                        }`}
                      >
                        <Play size={16} />
                        重启任务
                      </button>
                    )}
                  </div>
                </>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
