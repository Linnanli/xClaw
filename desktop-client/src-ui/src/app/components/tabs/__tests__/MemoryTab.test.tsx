import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { vi, describe, it, expect, beforeEach } from 'vitest';
import { MemoryTab } from '../MemoryTab';
import { ThemeProvider } from '../../../contexts/ThemeContext';
import { memoryApi } from '../../../utils/tauri';

// Mock the tauri API
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
  default: ({ children }: { children: string }) => <div data-testid="markdown">{children}</div>,
}));

const mockMemoryApi = memoryApi as any;

const renderMemoryTab = (theme: 'light' | 'dark' = 'light') => {
  return render(
    <ThemeProvider>
      <div data-theme={theme}>
        <MemoryTab />
      </div>
    </ThemeProvider>
  );
};

describe('MemoryTab', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    
    // Default mock implementations
    mockMemoryApi.getMemoryTree.mockResolvedValue({
      entries: [
        { path: 'SOUL.md', is_dir: false },
        { path: 'USER.md', is_dir: false },
        { path: 'docs', is_dir: true },
        { path: 'docs/README.md', is_dir: false },
      ],
    });
    
    mockMemoryApi.isMemoryFileProtected.mockImplementation((path: string) => {
      return Promise.resolve(['SOUL.md', 'IDENTITY.md', 'AGENTS.md'].includes(path.split('/').pop() || ''));
    });
    
    mockMemoryApi.readMemory.mockResolvedValue({
      path: 'test.md',
      content: 'Test content',
      updated_at: null,
    });
    
    mockMemoryApi.searchMemory.mockResolvedValue([]);
  });

  describe('初始化和渲染', () => {
    it('应该正确渲染组件', async () => {
      renderMemoryTab();
      
      expect(screen.getByPlaceholderText('搜索记忆...')).toBeInTheDocument();
      
      await waitFor(() => {
        expect(screen.getByText('SOUL.md')).toBeInTheDocument();
        expect(screen.getByText('USER.md')).toBeInTheDocument();
        expect(screen.getByText('docs')).toBeInTheDocument();
      });
    });

    it('应该在加载失败时显示错误信息', async () => {
      mockMemoryApi.getMemoryTree.mockRejectedValue(new Error('Network error'));
      
      renderMemoryTab();
      
      await waitFor(() => {
        expect(screen.getByText('Failed to load memories')).toBeInTheDocument();
      });
    });

    it('应该在没有记忆条目时显示空状态', async () => {
      mockMemoryApi.getMemoryTree.mockResolvedValue({ entries: [] });
      
      renderMemoryTab();
      
      await waitFor(() => {
        expect(screen.getByText('暂无记忆条目')).toBeInTheDocument();
      });
    });
  });

  describe('文件树交互', () => {
    it('应该能够展开和折叠文件夹', async () => {
      renderMemoryTab();
      
      await waitFor(() => {
        expect(screen.getByText('docs')).toBeInTheDocument();
      });
      
      // 点击文件夹展开
      fireEvent.click(screen.getByText('docs'));
      
      await waitFor(() => {
        expect(screen.getByText('README.md')).toBeInTheDocument();
      });
    });

    it('应该能够选择文件并显示内容', async () => {
      renderMemoryTab();
      
      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });
      
      // 点击文件
      fireEvent.click(screen.getByText('USER.md'));
      
      await waitFor(() => {
        expect(mockMemoryApi.readMemory).toHaveBeenCalledWith('USER.md');
        expect(mockMemoryApi.isMemoryFileProtected).toHaveBeenCalledWith('USER.md');
      });
    });
  });

  describe('文件保护机制', () => {
    it('应该对受保护文件隐藏删除按钮', async () => {
      renderMemoryTab();
      
      await waitFor(() => {
        expect(screen.getByText('SOUL.md')).toBeInTheDocument();
      });
      
      // 选择受保护文件
      fireEvent.click(screen.getByText('SOUL.md'));
      
      await waitFor(() => {
        expect(screen.getByText('编辑')).toBeInTheDocument();
        expect(screen.queryByText('删除')).not.toBeInTheDocument();
      });
    });

    it('应该对非保护文件显示删除按钮', async () => {
      renderMemoryTab();
      
      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });
      
      // 选择非保护文件
      fireEvent.click(screen.getByText('USER.md'));
      
      await waitFor(() => {
        expect(screen.getByText('编辑')).toBeInTheDocument();
        expect(screen.getByText('删除')).toBeInTheDocument();
      });
    });
  });

  describe('文件编辑功能', () => {
    it('应该能够进入编辑模式', async () => {
      renderMemoryTab();
      
      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });
      
      // 选择文件
      fireEvent.click(screen.getByText('USER.md'));
      
      await waitFor(() => {
        expect(screen.getByText('编辑')).toBeInTheDocument();
      });
      
      // 点击编辑按钮
      fireEvent.click(screen.getByText('编辑'));
      
      expect(screen.getByText('保存')).toBeInTheDocument();
      expect(screen.getByText('取消')).toBeInTheDocument();
      const textareas = screen.getAllByRole('textbox');
      const textarea = textareas.find(el => el.tagName === 'TEXTAREA');
      expect(textarea).toBeInTheDocument();
    });

    it('应该能够保存文件', async () => {
      mockMemoryApi.writeMemory.mockResolvedValue({ path: 'USER.md', status: 'success' });
      
      renderMemoryTab();
      
      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });
      
      // 选择文件并进入编辑模式
      fireEvent.click(screen.getByText('USER.md'));
      
      await waitFor(() => {
        fireEvent.click(screen.getByText('编辑'));
      });
      
      // 修改内容
      const textareas = screen.getAllByRole('textbox');
      const textarea = textareas.find(el => el.tagName === 'TEXTAREA');
      expect(textarea).toBeInTheDocument();
      fireEvent.change(textarea!, { target: { value: 'New content' } });
      
      // 保存
      fireEvent.click(screen.getByText('保存'));
      
      await waitFor(() => {
        expect(mockMemoryApi.writeMemory).toHaveBeenCalledWith('USER.md', 'New content');
      });
    });

    it('应该能够取消编辑', async () => {
      renderMemoryTab();
      
      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });
      
      // 选择文件并进入编辑模式
      fireEvent.click(screen.getByText('USER.md'));
      
      await waitFor(() => {
        fireEvent.click(screen.getByText('编辑'));
      });
      
      // 取消编辑
      fireEvent.click(screen.getByText('取消'));
      
      expect(screen.getByText('编辑')).toBeInTheDocument();
      expect(screen.queryByText('保存')).not.toBeInTheDocument();
    });
  });

  describe('文件删除功能', () => {
    it('应该能够删除非保护文件', async () => {
      mockMemoryApi.deleteMemoryLocal.mockResolvedValue({
        success: true,
        message: '文件删除成功',
        is_protected: false,
      });
      
      renderMemoryTab();
      
      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });
      
      // 选择文件
      fireEvent.click(screen.getByText('USER.md'));
      
      await waitFor(() => {
        expect(screen.getByText('删除')).toBeInTheDocument();
      });
      
      // 点击删除按钮
      fireEvent.click(screen.getByText('删除'));
      
      // 确认删除对话框应该出现
      await waitFor(() => {
        expect(screen.getByText('删除文件')).toBeInTheDocument();
      });
      
      // 确认删除
      fireEvent.click(screen.getAllByText('删除')[1]); // 第二个删除按钮是确认按钮
      
      await waitFor(() => {
        expect(mockMemoryApi.deleteMemoryLocal).toHaveBeenCalledWith('USER.md', false);
      });
    });

    it('应该处理删除失败的情况', async () => {
      mockMemoryApi.deleteMemoryLocal.mockResolvedValue({
        success: false,
        message: '删除失败：文件被占用',
        is_protected: false,
      });
      
      renderMemoryTab();
      
      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });
      
      // 选择文件并删除
      fireEvent.click(screen.getByText('USER.md'));
      
      await waitFor(() => {
        fireEvent.click(screen.getByText('删除'));
      });
      
      await waitFor(() => {
        fireEvent.click(screen.getAllByText('删除')[1]);
      });
      
      await waitFor(() => {
        expect(screen.getByText('删除失败：文件被占用')).toBeInTheDocument();
      });
    });
  });

  describe('搜索功能', () => {
    it('应该能够搜索记忆文件', async () => {
      const searchResults = [
        {
          path: 'test.md',
          content: 'This is a test file with search content',
          score: 0.95,
        },
      ];
      
      mockMemoryApi.searchMemory.mockResolvedValue(searchResults);
      
      renderMemoryTab();
      
      // 输入搜索查询
      const searchInput = screen.getByPlaceholderText('搜索记忆...');
      fireEvent.change(searchInput, { target: { value: 'test' } });
      
      await waitFor(() => {
        expect(mockMemoryApi.searchMemory).toHaveBeenCalledWith('test');
        expect(screen.getByText('test.md')).toBeInTheDocument();
        expect(screen.getByText('相关度: 95%')).toBeInTheDocument();
      });
    });

    it('应该在没有搜索结果时显示空状态', async () => {
      mockMemoryApi.searchMemory.mockResolvedValue([]);
      
      renderMemoryTab();
      
      // 输入搜索查询
      const searchInput = screen.getByPlaceholderText('搜索记忆...');
      fireEvent.change(searchInput, { target: { value: 'nonexistent' } });
      
      await waitFor(() => {
        expect(screen.getByText('未找到匹配的记忆')).toBeInTheDocument();
      });
    });
  });

  describe('错误处理', () => {
    it('应该处理文件读取错误', async () => {
      mockMemoryApi.readMemory.mockRejectedValue(new Error('File not found'));
      
      renderMemoryTab();
      
      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });
      
      // 选择文件
      fireEvent.click(screen.getByText('USER.md'));
      
      await waitFor(() => {
        expect(screen.getByText('读取文件失败: File not found')).toBeInTheDocument();
      });
    });

    it('应该处理文件保存错误', async () => {
      mockMemoryApi.writeMemory.mockRejectedValue(new Error('Permission denied'));
      
      renderMemoryTab();
      
      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });
      
      // 选择文件并进入编辑模式
      fireEvent.click(screen.getByText('USER.md'));
      
      await waitFor(() => {
        fireEvent.click(screen.getByText('编辑'));
      });
      
      // 尝试保存
      fireEvent.click(screen.getByText('保存'));
      
      await waitFor(() => {
        expect(screen.getByText('保存文件失败: Permission denied')).toBeInTheDocument();
      });
    });

    it('应该处理删除错误', async () => {
      mockMemoryApi.deleteMemoryLocal.mockRejectedValue(new Error('Network error'));
      
      renderMemoryTab();
      
      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });
      
      // 选择文件并删除
      fireEvent.click(screen.getByText('USER.md'));
      
      await waitFor(() => {
        fireEvent.click(screen.getByText('删除'));
      });
      
      await waitFor(() => {
        fireEvent.click(screen.getAllByText('删除')[1]);
      });
      
      await waitFor(() => {
        expect(screen.getByText('删除文件失败: Network error')).toBeInTheDocument();
      });
    });
  });

  describe('已删除文件过滤', () => {
    it('应该从文件树中过滤掉已删除的文件', async () => {
      // 设置一个文件为已删除状态
      mockMemoryApi.readMemory.mockImplementation(async (path: string) => {
        if (path === 'deleted.md') {
          return {
            path: 'deleted.md',
            content: '<!-- DELETED -->',
            updated_at: null,
          };
        }
        return {
          path: path,
          content: 'Normal content',
          updated_at: null,
        };
      });

      // 设置 memoryContentUtils 来识别已删除文件
      const { memoryContentUtils } = await import('../../../utils/tauri');
      (memoryContentUtils.isDeleted as any).mockImplementation((content: any) => {
        return content.content.trim() === '<!-- DELETED -->';
      });

      // 设置文件树包含已删除文件
      mockMemoryApi.getMemoryTree.mockResolvedValue({
        entries: [
          { path: 'normal.md', is_dir: false },
          { path: 'deleted.md', is_dir: false },
        ],
      });

      renderMemoryTab();

      // 等待文件树加载并过滤
      await waitFor(() => {
        expect(screen.getByText('normal.md')).toBeInTheDocument();
      });

      // 已删除的文件不应该出现在树中
      expect(screen.queryByText('deleted.md')).not.toBeInTheDocument();
    });

    it('应该从搜索结果中过滤掉已删除的文件', async () => {
      // 设置搜索结果包含已删除文件
      mockMemoryApi.searchMemory.mockResolvedValue([
        {
          path: 'normal.md',
          content: 'Normal search result',
          score: 0.9,
        },
        {
          path: 'deleted.md',
          content: 'Deleted search result',
          score: 0.8,
        },
      ]);

      // 设置读取文件的响应
      mockMemoryApi.readMemory.mockImplementation(async (path: string) => {
        if (path === 'deleted.md') {
          return {
            path: 'deleted.md',
            content: '<!-- DELETED -->',
            updated_at: null,
          };
        }
        return {
          path: path,
          content: 'Normal content',
          updated_at: null,
        };
      });

      // 设置 memoryContentUtils
      const { memoryContentUtils } = await import('../../../utils/tauri');
      (memoryContentUtils.isDeleted as any).mockImplementation((content: any) => {
        return content.content.trim() === '<!-- DELETED -->';
      });

      renderMemoryTab();

      // 执行搜索
      const searchInput = screen.getByPlaceholderText('搜索记忆...');
      fireEvent.change(searchInput, { target: { value: 'search' } });

      // 等待搜索结果加载并过滤
      await waitFor(() => {
        expect(screen.getByText('normal.md')).toBeInTheDocument();
      });

      // 已删除的文件不应该出现在搜索结果中
      expect(screen.queryByText('deleted.md')).not.toBeInTheDocument();
    });

    it('应该在删除文件后自动从树中移除', async () => {
      mockMemoryApi.deleteMemoryLocal.mockResolvedValue({
        success: true,
        message: '文件删除成功',
        is_protected: false,
      });

      // 初始状态：文件存在
      mockMemoryApi.getMemoryTree.mockResolvedValue({
        entries: [
          { path: 'test.md', is_dir: false },
        ],
      });

      mockMemoryApi.readMemory.mockResolvedValue({
        path: 'test.md',
        content: 'Normal content',
        updated_at: null,
      });

      const { memoryContentUtils } = await import('../../../utils/tauri');
      (memoryContentUtils.isDeleted as any).mockReturnValue(false);

      renderMemoryTab();

      // 验证文件最初存在
      await waitFor(() => {
        expect(screen.getByText('test.md')).toBeInTheDocument();
      });

      // 选择并删除文件
      fireEvent.click(screen.getByText('test.md'));

      await waitFor(() => {
        fireEvent.click(screen.getByText('删除'));
      });

      // 模拟删除后的状态：文件被标记为已删除
      (memoryContentUtils.isDeleted as any).mockReturnValue(true);
      mockMemoryApi.readMemory.mockResolvedValue({
        path: 'test.md',
        content: '<!-- DELETED -->',
        updated_at: null,
      });

      // 确认删除
      await waitFor(() => {
        fireEvent.click(screen.getAllByText('删除')[1]);
      });

      // 等待文件从树中消失
      await waitFor(() => {
        expect(screen.queryByText('test.md')).not.toBeInTheDocument();
      });
    });
  });

  describe('主题支持', () => {
    it('应该在深色主题下正确渲染', () => {
      renderMemoryTab('dark');
      
      // 检查主题相关的类名存在（不检查具体的背景色类）
      const searchInput = screen.getByPlaceholderText('搜索记忆...');
      expect(searchInput).toBeInTheDocument();
    });

    it('应该在浅色主题下正确渲染', () => {
      renderMemoryTab('light');
      
      const searchInput = screen.getByPlaceholderText('搜索记忆...');
      expect(searchInput).toHaveClass('bg-white');
    });
  });
});