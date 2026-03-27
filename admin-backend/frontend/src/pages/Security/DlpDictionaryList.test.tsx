import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { BrowserRouter } from 'react-router-dom';
import { DlpDictionaryList } from './DlpDictionaryList';
import { apiClient } from '../../api/client';
import { clickPopconfirmOk } from '../../test/setup';

vi.mock('../../api/client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

vi.mock('antd', async () => {
  const actual = await vi.importActual('antd');
  return {
    ...actual,
    message: {
      success: vi.fn(),
      error: vi.fn(),
      warning: vi.fn(),
    },
  };
});

const mockDictionaries = [
  {
    id: 'dict-1',
    name: '敏感词字典',
    description: '包含常见敏感词',
    keywords: ['机密', '绝密', '内部'],
    keyword_count: 3,
    created_at: '2026-03-20T10:00:00Z',
    updated_at: '2026-03-20T10:00:00Z',
  },
  {
    id: 'dict-2',
    name: '政治敏感词',
    description: '政治相关敏感词汇',
    keywords: ['词汇A', '词汇B', '词汇C', '词汇D', '词汇E', '词汇F'],
    keyword_count: 6,
    created_at: '2026-03-21T10:00:00Z',
    updated_at: '2026-03-21T10:00:00Z',
  },
];

const renderComponent = () => {
  return render(
    <BrowserRouter>
      <DlpDictionaryList />
    </BrowserRouter>
  );
};

describe('DlpDictionaryList Component', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // ============================================================
  // 单元测试：组件渲染
  // ============================================================
  describe('Unit Tests - Component Rendering', () => {
    it('should render page title and action buttons', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { dictionaries: [] } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('字典管理')).toBeInTheDocument();
        expect(screen.getByText('刷新')).toBeInTheDocument();
        expect(screen.getByText('创建字典')).toBeInTheDocument();
      });
    });

    it('should render search bar', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { dictionaries: [] } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByPlaceholderText('搜索字典名称或描述')).toBeInTheDocument();
      });
    });

    it('should render table columns', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { dictionaries: mockDictionaries } });
      renderComponent();

      await waitFor(() => {
        const headers = screen.getAllByRole('columnheader');
        const headerTexts = headers.map(h => h.textContent);
        expect(headerTexts).toContain('字典名称');
        expect(headerTexts).toContain('描述');
        expect(headerTexts).toContain('关键字数量');
        expect(headerTexts).toContain('关键字预览');
        expect(headerTexts).toContain('更新时间');
        expect(headerTexts).toContain('操作');
      });
    });

    it('should render empty state when no dictionaries', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { dictionaries: [] } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText(/还没有字典/)).toBeInTheDocument();
        expect(screen.getByText('创建第一个字典')).toBeInTheDocument();
      });
    });
  });

  // ============================================================
  // 集成测试：数据加载和交互
  // ============================================================
  describe('Integration Tests - Data Loading', () => {
    it('should load dictionaries from API on mount', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { dictionaries: mockDictionaries } });
      renderComponent();

      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalledWith('/dlp-dictionaries');
      });
    });

    it('should display loaded dictionaries', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { dictionaries: mockDictionaries } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('敏感词字典')).toBeInTheDocument();
        expect(screen.getByText('政治敏感词')).toBeInTheDocument();
      });
    });

    it('should display dictionary count summary', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { dictionaries: mockDictionaries } });
      renderComponent();

      await waitFor(() => {
        const matches = screen.getAllByText(/共 2 个字典/);
        expect(matches.length).toBeGreaterThanOrEqual(1);
      });
    });

    it('should display keyword count tags', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { dictionaries: mockDictionaries } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('3 个')).toBeInTheDocument();
        expect(screen.getByText('6 个')).toBeInTheDocument();
      });
    });

    it('should display keyword preview with overflow', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { dictionaries: mockDictionaries } });
      renderComponent();

      await waitFor(() => {
        // dict-1 有 3 个关键字，全部显示
        expect(screen.getByText('机密')).toBeInTheDocument();
        expect(screen.getByText('绝密')).toBeInTheDocument();
        expect(screen.getByText('内部')).toBeInTheDocument();
        // dict-2 有 6 个关键字，显示前 5 个 + "+1"
        expect(screen.getByText('+1')).toBeInTheDocument();
      });
    });

    it('should reload dictionaries on refresh button click', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { dictionaries: mockDictionaries } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalledTimes(1);
      });

      await user.click(screen.getByText('刷新'));

      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalledTimes(2);
      });
    });
  });

  // ============================================================
  // 集成测试：搜索
  // ============================================================
  describe('Integration Tests - Search', () => {
    it('should filter dictionaries by search text', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { dictionaries: mockDictionaries } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('敏感词字典')).toBeInTheDocument();
      });

      const searchInput = screen.getByPlaceholderText('搜索字典名称或描述');
      await user.type(searchInput, '政治');

      await waitFor(() => {
        expect(screen.queryByText('敏感词字典')).not.toBeInTheDocument();
        expect(screen.getByText('政治敏感词')).toBeInTheDocument();
      });
    });

    it('should search by description', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { dictionaries: mockDictionaries } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('敏感词字典')).toBeInTheDocument();
      });

      const searchInput = screen.getByPlaceholderText('搜索字典名称或描述');
      await user.type(searchInput, '常见');

      await waitFor(() => {
        expect(screen.getByText('敏感词字典')).toBeInTheDocument();
        expect(screen.queryByText('政治敏感词')).not.toBeInTheDocument();
      });
    });

    it('should show filtered count', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { dictionaries: mockDictionaries } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('敏感词字典')).toBeInTheDocument();
      });

      const searchInput = screen.getByPlaceholderText('搜索字典名称或描述');
      await user.type(searchInput, '政治');

      await waitFor(() => {
        expect(screen.getByText(/显示 1 个/)).toBeInTheDocument();
      });
    });
  });

  // ============================================================
  // 集成测试：CRUD 操作
  // ============================================================
  describe('Integration Tests - CRUD Operations', () => {
    it('should open create modal on button click', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { dictionaries: mockDictionaries } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('敏感词字典')).toBeInTheDocument();
      });

      // 点击页面头部的"创建字典"按钮（图标按钮）
      const createBtns = screen.getAllByRole('button');
      const createBtn = createBtns.find(btn => btn.textContent?.includes('创建字典'));
      expect(createBtn).toBeDefined();
      await user.click(createBtn!);

      // antd Modal 渲染到 document.body 的 portal 中
      await waitFor(() => {
        const modalTitle = document.querySelector('.ant-modal-title');
        expect(modalTitle).not.toBeNull();
      }, { timeout: 5000 });
    });

    it('should open edit modal on edit button click', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { dictionaries: mockDictionaries } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('敏感词字典')).toBeInTheDocument();
      });

      const editButtons = screen.getAllByText('编辑');
      await user.click(editButtons[0]);

      await waitFor(() => {
        const modalTitle = document.querySelector('.ant-modal-title');
        expect(modalTitle?.textContent).toBe('编辑字典');
      });
    });

    it('should open edit modal on dictionary name click', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { dictionaries: mockDictionaries } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('敏感词字典')).toBeInTheDocument();
      });

      await user.click(screen.getByText('敏感词字典'));

      await waitFor(() => {
        const modalTitle = document.querySelector('.ant-modal-title');
        expect(modalTitle?.textContent).toBe('编辑字典');
      });
    });

    it('should delete dictionary after confirmation', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { dictionaries: mockDictionaries } });
      vi.mocked(apiClient.delete).mockResolvedValue({ data: {} });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('敏感词字典')).toBeInTheDocument();
      });

      const deleteButtons = screen.getAllByText('删除');
      await user.click(deleteButtons[0]);

      await waitFor(() => {
        expect(screen.getByText('确认删除')).toBeInTheDocument();
      });

      await clickPopconfirmOk();

      await waitFor(() => {
        expect(apiClient.delete).toHaveBeenCalledWith('/dlp-dictionaries/dict-1');
      });
    });
  });

  // ============================================================
  // 失败路径测试
  // ============================================================
  describe('Failure Path Tests', () => {
    it('test_failure_load_api_error: should handle load API error', async () => {
      const { message } = await import('antd');
      vi.mocked(apiClient.get).mockRejectedValue({
        response: { data: { error: '服务器错误' } },
      });

      renderComponent();

      await waitFor(() => {
        expect(message.error).toHaveBeenCalled();
      });
    });

    it('test_failure_load_network_error: should handle network error', async () => {
      const { message } = await import('antd');
      vi.mocked(apiClient.get).mockRejectedValue(new Error('Network Error'));

      renderComponent();

      await waitFor(() => {
        expect(message.error).toHaveBeenCalled();
      });
    });

    it('test_failure_delete_error: should handle delete API error', async () => {
      const { message } = await import('antd');
      vi.mocked(apiClient.get).mockResolvedValue({ data: { dictionaries: mockDictionaries } });
      vi.mocked(apiClient.delete).mockRejectedValue({
        response: { data: { error: '删除失败' } },
      });

      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('敏感词字典')).toBeInTheDocument();
      });

      const deleteButtons = screen.getAllByText('删除');
      await user.click(deleteButtons[0]);

      await waitFor(() => {
        expect(screen.getByText('确认删除')).toBeInTheDocument();
      });

      await clickPopconfirmOk();

      await waitFor(() => {
        expect(message.error).toHaveBeenCalled();
      });
    });

    it('test_failure_empty_dictionaries_response: should handle empty dictionaries array', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { dictionaries: [] } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText(/还没有字典/)).toBeInTheDocument();
      });
    });

    it('test_failure_null_dictionaries_response: should handle null dictionaries in response', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: {} });
      renderComponent();

      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalled();
      });
    });
  });

  // ============================================================
  // 安全测试
  // ============================================================
  describe('Security Tests', () => {
    it('test_security_xss_in_dict_name: should safely render dictionary names', async () => {
      const xssDicts = [{
        ...mockDictionaries[0],
        name: '<script>alert("xss")</script>',
      }];
      vi.mocked(apiClient.get).mockResolvedValue({ data: { dictionaries: xssDicts } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('<script>alert("xss")</script>')).toBeInTheDocument();
        expect(document.querySelector('script')).toBeNull();
      });
    });

    it('test_security_xss_in_keywords: should safely render keyword tags', async () => {
      const xssDicts = [{
        ...mockDictionaries[0],
        keywords: ['<img onerror=alert(1)>', 'normal'],
        keyword_count: 2,
      }];
      vi.mocked(apiClient.get).mockResolvedValue({ data: { dictionaries: xssDicts } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('<img onerror=alert(1)>')).toBeInTheDocument();
        expect(document.querySelector('img')).toBeNull();
      });
    });

    it('test_security_xss_in_search: should safely handle search input', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { dictionaries: mockDictionaries } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('敏感词字典')).toBeInTheDocument();
      });

      const searchInput = screen.getByPlaceholderText('搜索字典名称或描述');
      await user.type(searchInput, '<img onerror=alert(1)>');

      expect(document.querySelector('img')).toBeNull();
    });
  });

  // ============================================================
  // 契约测试
  // ============================================================
  describe('Contract Tests', () => {
    it('test_contract_load_dictionaries_endpoint: should call correct API endpoint', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { dictionaries: [] } });
      renderComponent();

      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalledWith('/dlp-dictionaries');
      });
    });

    it('test_contract_delete_endpoint: should call correct delete endpoint', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { dictionaries: mockDictionaries } });
      vi.mocked(apiClient.delete).mockResolvedValue({ data: {} });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('敏感词字典')).toBeInTheDocument();
      });

      const deleteButtons = screen.getAllByText('删除');
      await user.click(deleteButtons[0]);

      // Popconfirm 必须渲染确认按钮，否则测试本身有问题
      await waitFor(() => {
        expect(screen.getByText('确认删除')).toBeInTheDocument();
      });

      const popconfirmOkBtn = await waitFor(() => {
        const btn = document.querySelector('.ant-popconfirm-buttons .ant-btn-primary') as HTMLElement;
        expect(btn).not.toBeNull();
        return btn;
      });
      await user.click(popconfirmOkBtn);

      await waitFor(() => {
        expect(apiClient.delete).toHaveBeenCalledWith('/dlp-dictionaries/dict-1');
      });
    });

    it('test_contract_api_response_format: should correctly parse dictionary from API', async () => {
      const dict = {
        id: 'dict-test',
        name: '测试字典',
        description: '测试描述',
        keywords: ['关键字1', '关键字2'],
        keyword_count: 2,
        created_at: '2026-03-21T10:00:00Z',
        updated_at: '2026-03-21T10:00:00Z',
      };
      vi.mocked(apiClient.get).mockResolvedValue({ data: { dictionaries: [dict] } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('测试字典')).toBeInTheDocument();
        expect(screen.getByText('测试描述')).toBeInTheDocument();
        expect(screen.getByText('2 个')).toBeInTheDocument();
        expect(screen.getByText('关键字1')).toBeInTheDocument();
        expect(screen.getByText('关键字2')).toBeInTheDocument();
      });
    });
  });

  // ============================================================
  // 需求级测试
  // ============================================================
  describe('Requirements Tests', () => {
    it('req_dict_001: should display dictionary list with all columns', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { dictionaries: mockDictionaries } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('字典管理')).toBeInTheDocument();
        expect(screen.getByText('敏感词字典')).toBeInTheDocument();
      });
    });

    it('req_dict_002: should support search functionality', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { dictionaries: mockDictionaries } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByPlaceholderText('搜索字典名称或描述')).toBeInTheDocument();
      });
    });

    it('req_dict_003: should support create functionality', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { dictionaries: [] } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('创建字典')).toBeInTheDocument();
      });
    });

    it('req_dict_004: should support edit functionality', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { dictionaries: mockDictionaries } });
      renderComponent();

      await waitFor(() => {
        const editButtons = screen.getAllByText('编辑');
        expect(editButtons.length).toBeGreaterThan(0);
      });
    });

    it('req_dict_005: should support delete with confirmation', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { dictionaries: mockDictionaries } });
      renderComponent();

      await waitFor(() => {
        const deleteButtons = screen.getAllByText('删除');
        expect(deleteButtons.length).toBeGreaterThan(0);
      });
    });
  });

  // ============================================================
  // 代码覆盖测试：边界情况
  // ============================================================
  describe('Code Coverage - Edge Cases', () => {
    it('should handle dictionary with no description', async () => {
      const noDescDict = [{
        ...mockDictionaries[0],
        description: null,
      }];
      vi.mocked(apiClient.get).mockResolvedValue({ data: { dictionaries: noDescDict } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('敏感词字典')).toBeInTheDocument();
        expect(screen.getByText('-')).toBeInTheDocument();
      });
    });

    it('should handle dictionary with empty keywords', async () => {
      const emptyKwDict = [{
        ...mockDictionaries[0],
        keywords: [],
        keyword_count: 0,
      }];
      vi.mocked(apiClient.get).mockResolvedValue({ data: { dictionaries: emptyKwDict } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('敏感词字典')).toBeInTheDocument();
        expect(screen.getByText('0 个')).toBeInTheDocument();
        expect(screen.getByText('无')).toBeInTheDocument();
      });
    });

    it('should handle dictionary with many keywords showing overflow', async () => {
      const manyKwDict = [{
        ...mockDictionaries[0],
        keywords: ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H'],
        keyword_count: 8,
      }];
      vi.mocked(apiClient.get).mockResolvedValue({ data: { dictionaries: manyKwDict } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('+3')).toBeInTheDocument();
      });
    });
  });
});
