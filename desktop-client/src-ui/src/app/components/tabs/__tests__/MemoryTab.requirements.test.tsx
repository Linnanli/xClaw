/**
 * MemoryTab 需求级测试（重写版）
 *
 * 覆盖维度：
 * - REQ_MEMORY_001: 文件树显示
 * - REQ_MEMORY_002: 文件内容查看
 * - REQ_MEMORY_003: 文件编辑功能
 * - REQ_MEMORY_004: 文件删除保护
 * - REQ_MEMORY_005: 文件删除功能
 * - REQ_MEMORY_006: 搜索功能
 * - REQ_MEMORY_007: 已删除文件处理
 * - REQ_MEMORY_008: 系统文件分区
 */

import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { vi, describe, it, expect, beforeEach } from 'vitest';
import { MemoryTab } from '../MemoryTab';
import { memoryApi, memoryContentUtils } from '../../../utils/tauri';

vi.mock('../../../utils/tauri', () => ({
  memoryApi: {
    getMemoryTree: vi.fn(),
    readMemory: vi.fn(),
    writeMemory: vi.fn(),
    deleteMemoryLocal: vi.fn(),
    isMemoryFileProtected: vi.fn(),
    searchMemory: vi.fn(),
  },
  memoryContentUtils: {
    isDeleted: vi.fn(),
    getActualContent: vi.fn(),
    shouldShowInTree: vi.fn(),
  },
}));

vi.mock('react-markdown', () => ({
  default: ({ children }: { children: string }) => (
    <div data-testid="markdown">{children}</div>
  ),
}));

vi.mock('../../common/DeleteConfirmDialog', () => ({
  DeleteConfirmDialog: ({
    isOpen,
    onConfirm,
    onCancel,
  }: {
    isOpen: boolean;
    title: string;
    message: string;
    onConfirm: () => void;
    onCancel: () => void;
  }) =>
    isOpen ? (
      <div data-testid="delete-dialog">
        <button onClick={onConfirm}>确认删除</button>
        <button onClick={onCancel}>取消删除</button>
      </div>
    ) : null,
}));

const mockMemoryApi = vi.mocked(memoryApi);
const mockContentUtils = vi.mocked(memoryContentUtils);

function renderMemoryTab() {
  return render(<MemoryTab />);
}

describe('MemoryTab - 需求级测试', () => {
  beforeEach(() => {
    vi.clearAllMocks();

    mockMemoryApi.getMemoryTree.mockResolvedValue({
      entries: [
        { path: 'docs', is_dir: true },
        { path: 'docs/README.md', is_dir: false },
        { path: 'docs/guide.md', is_dir: false },
        { path: 'USER.md', is_dir: false },
        { path: 'notes.md', is_dir: false },
        { path: 'SOUL.md', is_dir: false },
        { path: 'IDENTITY.md', is_dir: false },
      ],
    });

    mockMemoryApi.readMemory.mockResolvedValue({
      path: 'USER.md',
      content: '# User Notes\nSome content.',
      updated_at: '2025-03-20T10:00:00Z',
    });

    mockMemoryApi.isMemoryFileProtected.mockImplementation(async (path: string) => {
      const name = path.split('/').pop() || '';
      return ['SOUL.md', 'IDENTITY.md', 'AGENTS.md'].includes(name);
    });

    mockMemoryApi.writeMemory.mockResolvedValue({ path: '', status: 'success' });
    mockMemoryApi.deleteMemoryLocal.mockResolvedValue({
      success: true,
      message: 'Deleted',
      is_protected: false,
    });

    mockContentUtils.isDeleted.mockReturnValue(false);
    mockContentUtils.getActualContent.mockImplementation((c: any) => c.content);
    mockContentUtils.shouldShowInTree.mockReturnValue(true);
  });

  /* ── REQ_MEMORY_001: 文件树显示 ── */

  describe('req_memory_001_file_tree_display', () => {
    it('应该显示所有记忆文件的层级结构', async () => {
      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('docs')).toBeInTheDocument();
        expect(screen.getByText('USER.md')).toBeInTheDocument();
        expect(screen.getByText('notes.md')).toBeInTheDocument();
      });
    });

    it('应该支持文件夹展开和折叠', async () => {
      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('docs')).toBeInTheDocument();
      });

      // 初始折叠
      expect(screen.queryByText('README.md')).not.toBeInTheDocument();

      // 展开
      fireEvent.click(screen.getByText('docs'));
      expect(screen.getByText('README.md')).toBeInTheDocument();
      expect(screen.getByText('guide.md')).toBeInTheDocument();

      // 折叠
      fireEvent.click(screen.getByText('docs'));
      expect(screen.queryByText('README.md')).not.toBeInTheDocument();
    });
  });

  /* ── REQ_MEMORY_002: 文件内容查看 ── */

  describe('req_memory_002_file_content_view', () => {
    it('应该能选择文件并显示内容', async () => {
      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('USER.md'));

      await waitFor(() => {
        expect(mockMemoryApi.readMemory).toHaveBeenCalledWith('USER.md');
        expect(screen.getByText('编辑')).toBeInTheDocument();
      });
    });

    it('Markdown 文件应使用 Markdown 渲染', async () => {
      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('USER.md'));

      await waitFor(() => {
        expect(screen.getByTestId('markdown')).toBeInTheDocument();
      });
    });

    it('应该显示文件更新时间', async () => {
      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('USER.md'));

      await waitFor(() => {
        expect(screen.getByText(/最后更新/)).toBeInTheDocument();
      });
    });
  });

  /* ── REQ_MEMORY_003: 文件编辑功能 ── */

  describe('req_memory_003_file_editing', () => {
    it('应该能编辑并保存文件', async () => {
      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('USER.md'));
      await waitFor(() => {
        fireEvent.click(screen.getByText('编辑'));
      });

      const allTextboxes = screen.getAllByRole('textbox');
      const textarea = allTextboxes.find((el) => el.tagName === 'TEXTAREA')!;
      fireEvent.change(textarea, { target: { value: 'New content' } });
      fireEvent.click(screen.getByText('保存'));

      await waitFor(() => {
        expect(mockMemoryApi.writeMemory).toHaveBeenCalledWith('USER.md', 'New content');
      });
    });

    it('应该能取消编辑', async () => {
      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('USER.md'));
      await waitFor(() => {
        fireEvent.click(screen.getByText('编辑'));
      });

      fireEvent.click(screen.getByText('取消'));

      expect(screen.getByText('编辑')).toBeInTheDocument();
      expect(screen.queryByText('保存')).not.toBeInTheDocument();
    });
  });

  /* ── REQ_MEMORY_004: 文件删除保护 ── */

  describe('req_memory_004_delete_protection', () => {
    it('系统文件不应有删除按钮', async () => {
      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('SOUL.md')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('SOUL.md'));

      await waitFor(() => {
        expect(screen.getByText('编辑')).toBeInTheDocument();
        expect(screen.queryByText('删除')).not.toBeInTheDocument();
      });
    });

    it('普通文件应有删除按钮', async () => {
      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('USER.md'));

      await waitFor(() => {
        expect(screen.getByText('删除')).toBeInTheDocument();
      });
    });
  });

  /* ── REQ_MEMORY_005: 文件删除功能 ── */

  describe('req_memory_005_file_deletion', () => {
    it('应该通过确认对话框删除文件', async () => {
      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('USER.md'));
      await waitFor(() => {
        fireEvent.click(screen.getByText('删除'));
      });

      expect(screen.getByTestId('delete-dialog')).toBeInTheDocument();

      fireEvent.click(screen.getByText('确认删除'));

      await waitFor(() => {
        expect(mockMemoryApi.deleteMemoryLocal).toHaveBeenCalledWith('USER.md', false);
      });
    });

    it('应该能取消删除', async () => {
      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('USER.md'));
      await waitFor(() => {
        fireEvent.click(screen.getByText('删除'));
      });

      fireEvent.click(screen.getByText('取消删除'));

      expect(screen.queryByTestId('delete-dialog')).not.toBeInTheDocument();
      expect(mockMemoryApi.deleteMemoryLocal).not.toHaveBeenCalled();
    });
  });

  /* ── REQ_MEMORY_006: 搜索功能 ── */

  describe('req_memory_006_search', () => {
    it('应该根据文件名过滤', async () => {
      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });

      fireEvent.change(screen.getByPlaceholderText('搜索记忆文件...'), {
        target: { value: 'USER' },
      });

      expect(screen.getByText('USER.md')).toBeInTheDocument();
      expect(screen.queryByText('notes.md')).not.toBeInTheDocument();
    });

    it('搜索应自动展开匹配的文件夹', async () => {
      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('docs')).toBeInTheDocument();
      });

      fireEvent.change(screen.getByPlaceholderText('搜索记忆文件...'), {
        target: { value: 'README' },
      });

      expect(screen.getByText('README.md')).toBeInTheDocument();
    });

    it('无匹配时应显示空状态', async () => {
      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });

      fireEvent.change(screen.getByPlaceholderText('搜索记忆文件...'), {
        target: { value: 'zzzzz' },
      });

      expect(screen.getByText('未找到匹配文件')).toBeInTheDocument();
    });
  });

  /* ── REQ_MEMORY_007: 已删除文件处理 ── */

  describe('req_memory_007_deleted_files', () => {
    it('应该从文件树中过滤已删除文件', async () => {
      mockMemoryApi.getMemoryTree.mockResolvedValue({
        entries: [
          { path: 'active.md', is_dir: false },
          { path: 'deleted.md', is_dir: false },
        ],
      });

      mockMemoryApi.readMemory.mockImplementation(async (path: string) => {
        if (path === 'deleted.md') {
          return { path, content: '<!-- DELETED -->', updated_at: undefined };
        }
        return { path, content: 'Active', updated_at: undefined };
      });

      mockContentUtils.isDeleted.mockImplementation(
        (c: any) => c.content.trim() === '<!-- DELETED -->',
      );

      renderMemoryTab();

      await waitFor(() => {
        expect(screen.getByText('active.md')).toBeInTheDocument();
      });

      expect(screen.queryByText('deleted.md')).not.toBeInTheDocument();
    });
  });

  /* ── REQ_MEMORY_008: 系统文件分区 ── */

  describe('req_memory_008_system_files_section', () => {
    it('系统文件应在独立区域显示', async () => {
      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('系统文件')).toBeInTheDocument();
        expect(screen.getByText('SOUL.md')).toBeInTheDocument();
        expect(screen.getByText('IDENTITY.md')).toBeInTheDocument();
      });
    });

    it('普通文件不应在系统文件区域', async () => {
      renderMemoryTab();
      await waitFor(() => {
        // USER.md 和 notes.md 应该在普通区域
        expect(screen.getByText('USER.md')).toBeInTheDocument();
        expect(screen.getByText('notes.md')).toBeInTheDocument();
      });
    });
  });
});
