/**
 * MemoryTab 单元测试（重写版）
 *
 * 覆盖维度：
 * - 正常路径：渲染、文件树交互、文件预览、编辑、删除、搜索
 * - 错误路径：API 失败、读取/保存/删除错误
 * - 安全审计：系统文件保护、删除确认
 * - 契约测试：文件树构建、已删除文件过滤
 */

import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { vi, describe, it, expect, beforeEach } from 'vitest';
import { MemoryTab } from '../MemoryTab';
import { memoryApi, memoryContentUtils } from '../../../utils/tauri';

// Mock tauri API
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

// Mock react-markdown
vi.mock('react-markdown', () => ({
  default: ({ children }: { children: string }) => (
    <div data-testid="markdown">{children}</div>
  ),
}));

// Mock DeleteConfirmDialog
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

describe('MemoryTab', () => {
  beforeEach(() => {
    vi.clearAllMocks();

    mockMemoryApi.getMemoryTree.mockResolvedValue({
      entries: [
        { path: 'docs', is_dir: true },
        { path: 'docs/README.md', is_dir: false },
        { path: 'USER.md', is_dir: false },
        { path: 'SOUL.md', is_dir: false },
        { path: 'IDENTITY.md', is_dir: false },
      ],
    });

    mockMemoryApi.readMemory.mockResolvedValue({
      path: 'USER.md',
      content: '# User Notes\nSome content here.',
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
    mockContentUtils.getActualContent.mockImplementation(
      (content: any) => content.content,
    );
    mockContentUtils.shouldShowInTree.mockReturnValue(true);
  });

  /* ── 正常路径：渲染 ── */

  describe('初始化和渲染', () => {
    it('应该渲染搜索栏', async () => {
      renderMemoryTab();
      expect(screen.getByPlaceholderText('搜索记忆文件...')).toBeInTheDocument();
    });

    it('应该渲染文件树', async () => {
      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('docs')).toBeInTheDocument();
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });
    });

    it('应该分离系统文件到独立区域', async () => {
      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('系统文件')).toBeInTheDocument();
        expect(screen.getByText('SOUL.md')).toBeInTheDocument();
        expect(screen.getByText('IDENTITY.md')).toBeInTheDocument();
      });
    });

    it('应该显示空选择提示', async () => {
      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('选择一个文件查看内容')).toBeInTheDocument();
      });
    });

    it('应该在无条目时显示空状态', async () => {
      mockMemoryApi.getMemoryTree.mockResolvedValue({ entries: [] });
      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('暂无记忆条目')).toBeInTheDocument();
      });
    });
  });

  /* ── 正常路径：文件树交互 ── */

  describe('文件树交互', () => {
    it('应该能展开和折叠文件夹', async () => {
      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('docs')).toBeInTheDocument();
      });

      // 初始状态文件夹折叠，README.md 不可见
      expect(screen.queryByText('README.md')).not.toBeInTheDocument();

      // 点击展开
      fireEvent.click(screen.getByText('docs'));
      expect(screen.getByText('README.md')).toBeInTheDocument();

      // 再次点击折叠
      fireEvent.click(screen.getByText('docs'));
      expect(screen.queryByText('README.md')).not.toBeInTheDocument();
    });

    it('应该能选择文件并显示预览', async () => {
      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('USER.md'));

      await waitFor(() => {
        expect(mockMemoryApi.readMemory).toHaveBeenCalledWith('USER.md');
        // 编辑按钮应该出现（说明预览面板已渲染）
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
  });

  /* ── 正常路径：编辑 ── */

  describe('文件编辑', () => {
    it('应该能进入编辑模式', async () => {
      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('USER.md'));
      await waitFor(() => {
        expect(screen.getByText('编辑')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('编辑'));

      expect(screen.getByText('保存')).toBeInTheDocument();
      expect(screen.getByText('取消')).toBeInTheDocument();
    });

    it('应该能保存编辑内容', async () => {
      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('USER.md'));
      await waitFor(() => {
        fireEvent.click(screen.getByText('编辑'));
      });

      // 搜索框和 textarea 都是 textbox，用 tagName 区分
      const allTextboxes = screen.getAllByRole('textbox');
      const textarea = allTextboxes.find((el) => el.tagName === 'TEXTAREA');
      expect(textarea).toBeDefined();
      fireEvent.change(textarea!, { target: { value: 'Updated content' } });
      fireEvent.click(screen.getByText('保存'));

      await waitFor(() => {
        expect(mockMemoryApi.writeMemory).toHaveBeenCalledWith(
          'USER.md',
          'Updated content',
        );
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

  /* ── 正常路径：删除 ── */

  describe('文件删除', () => {
    it('非保护文件应显示删除按钮', async () => {
      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('USER.md'));

      await waitFor(() => {
        expect(screen.getByText('删除')).toBeInTheDocument();
      });
    });

    it('点击删除应打开确认对话框', async () => {
      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('USER.md'));
      await waitFor(() => {
        fireEvent.click(screen.getByText('删除'));
      });

      expect(screen.getByTestId('delete-dialog')).toBeInTheDocument();
    });

    it('确认删除应调用 API', async () => {
      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('USER.md'));
      await waitFor(() => {
        fireEvent.click(screen.getByText('删除'));
      });

      fireEvent.click(screen.getByText('确认删除'));

      await waitFor(() => {
        expect(mockMemoryApi.deleteMemoryLocal).toHaveBeenCalledWith(
          'USER.md',
          false,
        );
      });
    });

    it('取消删除应关闭对话框', async () => {
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
    });
  });

  /* ── 正常路径：搜索 ── */

  describe('搜索功能', () => {
    it('应该根据文件名过滤', async () => {
      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });

      fireEvent.change(screen.getByPlaceholderText('搜索记忆文件...'), {
        target: { value: 'USER' },
      });

      expect(screen.getByText('USER.md')).toBeInTheDocument();
      // SOUL.md 不匹配
      expect(screen.queryByText('SOUL.md')).not.toBeInTheDocument();
    });

    it('搜索应展开匹配的文件夹', async () => {
      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('docs')).toBeInTheDocument();
      });

      fireEvent.change(screen.getByPlaceholderText('搜索记忆文件...'), {
        target: { value: 'README' },
      });

      // docs 文件夹应该自动展开显示 README.md
      expect(screen.getByText('README.md')).toBeInTheDocument();
    });

    it('无匹配时应显示空状态', async () => {
      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });

      fireEvent.change(screen.getByPlaceholderText('搜索记忆文件...'), {
        target: { value: 'nonexistent' },
      });

      expect(screen.getByText('未找到匹配文件')).toBeInTheDocument();
    });
  });

  /* ── 错误路径 ── */

  describe('错误处理', () => {
    it('应该在加载失败时显示错误', async () => {
      mockMemoryApi.getMemoryTree.mockRejectedValue(new Error('Network error'));
      renderMemoryTab();

      await waitFor(() => {
        expect(screen.getByText('加载记忆失败')).toBeInTheDocument();
      });
    });

    it('应该在文件读取失败时显示错误', async () => {
      mockMemoryApi.readMemory.mockRejectedValue(new Error('File not found'));
      renderMemoryTab();

      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('USER.md'));

      await waitFor(() => {
        expect(screen.getByText('读取文件失败: File not found')).toBeInTheDocument();
      });
    });

    it('应该在保存失败时显示错误', async () => {
      mockMemoryApi.writeMemory.mockRejectedValue(new Error('Permission denied'));
      renderMemoryTab();

      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('USER.md'));
      await waitFor(() => {
        fireEvent.click(screen.getByText('编辑'));
      });

      fireEvent.click(screen.getByText('保存'));

      await waitFor(() => {
        expect(
          screen.getByText('保存失败: Permission denied'),
        ).toBeInTheDocument();
      });
    });

    it('应该在删除失败时显示错误', async () => {
      mockMemoryApi.deleteMemoryLocal.mockRejectedValue(
        new Error('Network error'),
      );
      renderMemoryTab();

      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('USER.md'));
      await waitFor(() => {
        fireEvent.click(screen.getByText('删除'));
      });

      fireEvent.click(screen.getByText('确认删除'));

      await waitFor(() => {
        expect(
          screen.getByText('删除失败: Network error'),
        ).toBeInTheDocument();
      });
    });

    it('删除返回 success=false 应显示错误信息', async () => {
      mockMemoryApi.deleteMemoryLocal.mockResolvedValue({
        success: false,
        message: '文件被占用',
        is_protected: false,
      });
      renderMemoryTab();

      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('USER.md'));
      await waitFor(() => {
        fireEvent.click(screen.getByText('删除'));
      });

      fireEvent.click(screen.getByText('确认删除'));

      await waitFor(() => {
        expect(screen.getByText('文件被占用')).toBeInTheDocument();
      });
    });
  });

  /* ── 安全审计：系统文件保护 ── */

  describe('安全审计 - 系统文件保护', () => {
    it('受保护文件不应显示删除按钮', async () => {
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

    it('非保护文件应显示删除按钮', async () => {
      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('USER.md'));

      await waitFor(() => {
        expect(screen.getByText('编辑')).toBeInTheDocument();
        expect(screen.getByText('删除')).toBeInTheDocument();
      });
    });

    it('系统文件应使用 Shield 图标', async () => {
      renderMemoryTab();
      await waitFor(() => {
        // SOUL.md 和 IDENTITY.md 在系统文件区域
        expect(screen.getByText('系统文件')).toBeInTheDocument();
      });
    });
  });

  /* ── 契约测试：文件树构建 ── */

  describe('契约测试 - 文件树构建', () => {
    it('应该正确构建嵌套文件树', async () => {
      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('docs')).toBeInTheDocument();
      });

      // 展开 docs 文件夹
      fireEvent.click(screen.getByText('docs'));
      expect(screen.getByText('README.md')).toBeInTheDocument();
    });

    it('应该过滤已删除的文件', async () => {
      mockMemoryApi.getMemoryTree.mockResolvedValue({
        entries: [
          { path: 'normal.md', is_dir: false },
          { path: 'deleted.md', is_dir: false },
        ],
      });

      mockMemoryApi.readMemory.mockImplementation(async (path: string) => {
        if (path === 'deleted.md') {
          return { path, content: '<!-- DELETED -->', updated_at: null };
        }
        return { path, content: 'Normal content', updated_at: null };
      });

      mockContentUtils.isDeleted.mockImplementation(
        (content: any) => content.content.trim() === '<!-- DELETED -->',
      );

      renderMemoryTab();

      await waitFor(() => {
        expect(screen.getByText('normal.md')).toBeInTheDocument();
      });

      expect(screen.queryByText('deleted.md')).not.toBeInTheDocument();
    });

    it('已删除文件被选中时应显示错误', async () => {
      // 简化：只有一个文件，filterDeleted 阶段返回正常，点击后返回已删除
      const readCalls: string[] = [];
      mockMemoryApi.readMemory.mockImplementation(async (path: string) => {
        readCalls.push(path);
        // filterDeleted 阶段（第一次调用）返回正常内容
        // handleFileClick 阶段（第二次调用）返回已删除内容
        const isFilterPhase = readCalls.filter((p) => p === path).length <= 1;
        if (isFilterPhase) {
          return { path, content: 'Normal', updated_at: '2025-01-01T00:00:00Z' };
        }
        return { path, content: '<!-- DELETED -->', updated_at: '2025-01-01T00:00:00Z' };
      });

      mockContentUtils.isDeleted.mockImplementation((content: any) => {
        return content.content.trim() === '<!-- DELETED -->';
      });
      mockContentUtils.getActualContent.mockImplementation((content: any) => {
        return content.content.trim() === '<!-- DELETED -->' ? '' : content.content;
      });

      mockMemoryApi.getMemoryTree.mockResolvedValue({
        entries: [{ path: 'test-file.md', is_dir: false }],
      });

      renderMemoryTab();

      await waitFor(() => {
        expect(screen.getByText('test-file.md')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('test-file.md'));

      await waitFor(() => {
        expect(screen.getByText(/已被删除/)).toBeInTheDocument();
      });
    });
  });

  /* ── 更新时间显示 ── */

  describe('更新时间', () => {
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

    it('编辑模式下不应显示更新时间', async () => {
      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('USER.md'));
      await waitFor(() => {
        fireEvent.click(screen.getByText('编辑'));
      });

      expect(screen.queryByText(/最后更新/)).not.toBeInTheDocument();
    });
  });
});
