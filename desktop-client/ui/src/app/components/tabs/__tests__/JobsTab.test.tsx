import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { JobsTab } from '../JobsTab';
import { ThemeProvider } from '../../../contexts/ThemeContext';
import * as tauri from '../../../utils/tauri';

// Mock tauri event（useEngineReady 内部使用）
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

// Mock Tauri API
vi.mock('../../../utils/tauri', () => ({
  jobApi: {
    getJobs: vi.fn(),
    getJobDetail: vi.fn(),
    cancelJob: vi.fn(),
    restartJob: vi.fn(),
  },
}));

const mockJobs = [
  {
    id: 'job-1',
    title: 'Test Job 1',
    status: 'pending',
    created_at: '2024-01-01T00:00:00Z',
    updated_at: '2024-01-01T00:00:00Z',
  },
  {
    id: 'job-2',
    title: 'Test Job 2',
    status: 'in_progress',
    created_at: '2024-01-02T00:00:00Z',
    updated_at: '2024-01-02T00:00:00Z',
  },
  {
    id: 'job-3',
    title: 'Test Job 3',
    status: 'completed',
    created_at: '2024-01-03T00:00:00Z',
    updated_at: '2024-01-03T00:00:00Z',
  },
];

const mockJobDetail = {
  id: 'job-1',
  title: 'Test Job 1',
  status: 'pending',
  description: 'Test job description',
  source: 'direct',
  created_at: '2024-01-01T00:00:00Z',
  events: [],
};

function renderJobsTab() {
  return render(
    <ThemeProvider>
      <JobsTab />
    </ThemeProvider>
  );
}

describe('JobsTab', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should render loading state initially', () => {
    vi.mocked(tauri.jobApi.getJobs).mockImplementation(() => new Promise(() => {}));
    renderJobsTab();
    expect(screen.getByText('加载任务中...')).toBeInTheDocument();
  });

  it('should render job list after loading', async () => {
    vi.mocked(tauri.jobApi.getJobs).mockResolvedValue(mockJobs);
    renderJobsTab();

    await waitFor(() => {
      expect(screen.getByText('Test Job 1')).toBeInTheDocument();
      expect(screen.getByText('Test Job 2')).toBeInTheDocument();
      expect(screen.getByText('Test Job 3')).toBeInTheDocument();
    });
  });

  it('should display status counts correctly', async () => {
    vi.mocked(tauri.jobApi.getJobs).mockResolvedValue(mockJobs);
    renderJobsTab();

    await waitFor(() => {
      expect(screen.getByText('3')).toBeInTheDocument(); // total
      const counts = screen.getAllByText('1');
      expect(counts.length).toBeGreaterThanOrEqual(3); // pending, in_progress, completed
    });
  });

  it('should show empty state when no jobs', async () => {
    vi.mocked(tauri.jobApi.getJobs).mockResolvedValue([]);
    renderJobsTab();

    await waitFor(() => {
      expect(screen.getByText('暂无任务')).toBeInTheDocument();
    });
  });

  it('should show error message on fetch failure', async () => {
    vi.mocked(tauri.jobApi.getJobs).mockRejectedValue(new Error('Failed to fetch'));
    renderJobsTab();

    await waitFor(() => {
      expect(screen.getByText('Failed to load jobs')).toBeInTheDocument();
    });
  });

  it('should open job detail modal when clicking on a job', async () => {
    vi.mocked(tauri.jobApi.getJobs).mockResolvedValue(mockJobs);
    vi.mocked(tauri.jobApi.getJobDetail).mockResolvedValue(mockJobDetail);
    renderJobsTab();

    await waitFor(() => {
      expect(screen.getByText('Test Job 1')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Test Job 1'));

    await waitFor(() => {
      expect(tauri.jobApi.getJobDetail).toHaveBeenCalledWith('job-1');
      expect(screen.getByText('任务详情')).toBeInTheDocument();
    });
  });

  it('should cancel job when clicking cancel button', async () => {
    vi.mocked(tauri.jobApi.getJobs).mockResolvedValue(mockJobs);
    vi.mocked(tauri.jobApi.getJobDetail).mockResolvedValue(mockJobDetail);
    vi.mocked(tauri.jobApi.cancelJob).mockResolvedValue(undefined);
    renderJobsTab();

    await waitFor(() => {
      expect(screen.getByText('Test Job 1')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Test Job 1'));

    await waitFor(() => {
      expect(screen.getByText('任务详情')).toBeInTheDocument();
    });

    const cancelButton = screen.getByText('取消任务');
    fireEvent.click(cancelButton);

    await waitFor(() => {
      expect(tauri.jobApi.cancelJob).toHaveBeenCalledWith('job-1');
    });
  });

  it('should restart job when clicking restart button', async () => {
    const completedJob = { ...mockJobDetail, status: 'completed' };
    vi.mocked(tauri.jobApi.getJobs).mockResolvedValue([completedJob]);
    vi.mocked(tauri.jobApi.getJobDetail).mockResolvedValue(completedJob);
    vi.mocked(tauri.jobApi.restartJob).mockResolvedValue(undefined);
    renderJobsTab();

    await waitFor(() => {
      expect(screen.getByText('Test Job 1')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Test Job 1'));

    await waitFor(() => {
      expect(screen.getByText('任务详情')).toBeInTheDocument();
    });

    const restartButton = screen.getByText('重启任务');
    fireEvent.click(restartButton);

    await waitFor(() => {
      expect(tauri.jobApi.restartJob).toHaveBeenCalledWith('job-1');
    });
  });

  it('should filter jobs by status', async () => {
    vi.mocked(tauri.jobApi.getJobs).mockResolvedValue(mockJobs);
    renderJobsTab();

    await waitFor(() => {
      expect(screen.getByText('Test Job 1')).toBeInTheDocument();
    });

    // Click on "进行中" filter button (find by role and accessible name)
    const buttons = screen.getAllByRole('button');
    const inProgressButton = buttons.find(btn => 
      btn.textContent?.includes('进行中') && btn.textContent?.includes('1')
    );
    expect(inProgressButton).toBeDefined();
    fireEvent.click(inProgressButton!);

    await waitFor(() => {
      expect(screen.getByText('Test Job 2')).toBeInTheDocument();
      expect(screen.queryByText('Test Job 1')).not.toBeInTheDocument();
      expect(screen.queryByText('Test Job 3')).not.toBeInTheDocument();
    });
  });

  it('should refresh job list', async () => {
    vi.mocked(tauri.jobApi.getJobs).mockResolvedValue(mockJobs);
    renderJobsTab();

    await waitFor(() => {
      expect(screen.getByText('Test Job 1')).toBeInTheDocument();
    });

    const refreshButton = screen.getByLabelText('刷新');
    fireEvent.click(refreshButton);

    await waitFor(() => {
      expect(tauri.jobApi.getJobs).toHaveBeenCalledTimes(2);
    });
  });
});
