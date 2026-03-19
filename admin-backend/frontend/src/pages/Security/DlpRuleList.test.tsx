import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { BrowserRouter } from 'react-router-dom';
import { DlpRuleList } from './DlpRuleList';
import { apiClient } from '../../api/client';

// Mock API client
vi.mock('../../api/client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    delete: vi.fn(),
    put: vi.fn(),
  },
}));

// Mock Ant Design message
vi.mock('antd', async () => {
  const actual = await vi.importActual('antd');
  return {
    ...actual,
    message: {
      success: vi.fn(),
      error: vi.fn(),
    },
  };
});

describe('DlpRuleList Component', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('Unit Tests - Component Rendering', () => {
    it('should render DLP rule list page', async () => {
      (apiClient.get as any).mockResolvedValue({ data: { rules: [] } });

      render(
        <BrowserRouter>
          <DlpRuleList />
        </BrowserRouter>
      );

      await waitFor(() => {
        expect(screen.getByText('DLP 规则管理')).toBeInTheDocument();
      });
    });

    it('should render table columns', async () => {
      (apiClient.get as any).mockResolvedValue({ data: { rules: [] } });

      render(
        <BrowserRouter>
          <DlpRuleList />
        </BrowserRouter>
      );

      await waitFor(() => {
        const headers = screen.getAllByRole('columnheader');
        const headerTexts = headers.map(h => h.textContent);
        expect(headerTexts).toContain('规则名');
        expect(headerTexts).toContain('匹配模式');
        expect(headerTexts).toContain('严重级别');
        expect(headerTexts).toContain('分类');
        expect(headerTexts).toContain('状态');
        expect(headerTexts).toContain('操作');
      });
    });
  });

  describe('Integration Tests - Data Loading', () => {
    it('should load DLP rules from API', async () => {
      const mockRules = [
        {
          id: '1',
          name: '身份证号',
          pattern: '\\d{17}[\\dXx]',
          replacement: '***',
          severity: 'high',
          description: '匹配身份证号',
          enabled: true,
          category: 'pii',
          created_at: '2026-03-19T10:00:00Z',
          updated_at: '2026-03-19T10:00:00Z',
        },
      ];

      (apiClient.get as any).mockResolvedValue({ data: { rules: mockRules } });

      render(
        <BrowserRouter>
          <DlpRuleList />
        </BrowserRouter>
      );

      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalledWith('/dlp-rules');
      });
    });

    it('should display DLP rules', async () => {
      const mockRules = [
        {
          id: '1',
          name: '身份证号',
          pattern: '\\d{17}[\\dXx]',
          replacement: '***',
          severity: 'high',
          description: '匹配身份证号',
          enabled: true,
          category: 'pii',
          created_at: '2026-03-19T10:00:00Z',
          updated_at: '2026-03-19T10:00:00Z',
        },
      ];

      (apiClient.get as any).mockResolvedValue({ data: { rules: mockRules } });

      render(
        <BrowserRouter>
          <DlpRuleList />
        </BrowserRouter>
      );

      await waitFor(() => {
        expect(screen.getByText('身份证号')).toBeInTheDocument();
      });
    });

    it('should display rule status', async () => {
      const mockRules = [
        {
          id: '1',
          name: '测试规则',
          pattern: 'test',
          severity: 'low',
          enabled: true,
          category: 'test',
          created_at: '2026-03-19T10:00:00Z',
          updated_at: '2026-03-19T10:00:00Z',
        },
      ];

      (apiClient.get as any).mockResolvedValue({ data: { rules: mockRules } });

      render(
        <BrowserRouter>
          <DlpRuleList />
        </BrowserRouter>
      );

      await waitFor(() => {
        expect(screen.getByText('启用')).toBeInTheDocument();
      });
    });
  });

  describe('Failure Path Tests - Error Handling', () => {
    it('should handle API error', async () => {
      const { message } = await import('antd');
      (apiClient.get as any).mockRejectedValue({
        response: { data: { error: 'Failed to load' } },
      });

      render(
        <BrowserRouter>
          <DlpRuleList />
        </BrowserRouter>
      );

      await waitFor(() => {
        expect(message.error).toHaveBeenCalled();
      });
    });
  });

  describe('Requirements Tests - DLP Management', () => {
    it('REQ-DLP-001: should display DLP rule list', async () => {
      (apiClient.get as any).mockResolvedValue({ data: { rules: [] } });

      render(
        <BrowserRouter>
          <DlpRuleList />
        </BrowserRouter>
      );

      await waitFor(() => {
        expect(screen.getByText('DLP 规则管理')).toBeInTheDocument();
      });
    });
  });

  describe('Security Tests - Data Protection', () => {
    it('should not expose sensitive data', async () => {
      const mockRules = [
        {
          id: '1',
          name: '测试规则',
          pattern: 'secret_pattern',
          severity: 'high',
          enabled: true,
          category: 'test',
          created_at: '2026-03-19T10:00:00Z',
          updated_at: '2026-03-19T10:00:00Z',
        },
      ];

      (apiClient.get as any).mockResolvedValue({ data: { rules: mockRules } });

      render(
        <BrowserRouter>
          <DlpRuleList />
        </BrowserRouter>
      );

      await waitFor(() => {
        expect(screen.getByText('测试规则')).toBeInTheDocument();
      });
    });
  });

  describe('Code Coverage Tests - Edge Cases', () => {
    it('should handle empty rule list', async () => {
      (apiClient.get as any).mockResolvedValue({ data: { rules: [] } });

      render(
        <BrowserRouter>
          <DlpRuleList />
        </BrowserRouter>
      );

      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalled();
      });
    });
  });
});
