import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { vi, describe, it, expect, beforeEach } from 'vitest';
import { MemoryTab } from '../MemoryTab';
import { ThemeProvider } from '../../../contexts/ThemeContext';
import { memoryApi, memoryContentUtils } from '../../../utils/tauri';

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
const mockMemoryContentUtils = memoryContentUtils as any;

const renderMemoryTab = () => {
  return render(
    <ThemeProvider>
      <MemoryTab />
    </ThemeProvider>
  );
};

describe('MemoryTab - 需求级测试', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    
    // Default mock implementations
    mockMemoryApi.getMemoryTree.mockResolvedValue({
      entries: [
        { path: 'SOUL.md', is_dir: false },
        { path: 'IDENTITY.md', is_dir: false },
        { path: 'AGENTS.md', is_dir: false },
        { path: 'USER.md', is_dir: false },
        { path: 'notes.md', is_dir: false },
      ],
    });
    
    mockMemoryApi.isMemoryFileProtected.mockImplementation((path: string) => {
      const fileName = path.split('/').pop() || '';
      return Promise.resolve(['SOUL.md', 'IDENTITY.md', 'AGENTS.md'].includes(fileName));
    });
    
    mockMemoryApi.readMemory.mockResolvedValue({
      path: 'test.md',
      content: 'Test content',
      updated_at: null,
    });
    
    mockMemoryContentUtils.isDeleted.mockReturnValue(false);
    mockMemoryContentUtils.getActualContent.mockImplementation((content: any) => content.content);
    mockMemoryContentUtils.shouldShowInTree.mockReturnValue(true);
  });

  describe('REQ_MEMORY_001: 记忆文件树显示', () => {
    it('应该显示所有记忆文件的层级结构', async () => {
      renderMemoryTab();
      
      await waitFor(() => {
        expect(screen.getByText('SOUL.md')).toBeInTheDocument();
        expect(screen.getByText('IDENTITY.md')).toBeInTheDocument();
        expect(screen.getByText('AGENTS.md')).toBeInTheDocument();
        expect(screen.getByText('USER.md')).toBeInTheDocument();
        expect(screen.getByText('notes.md')).toBeInTheDocument();
      });
      
      expect(mockMemoryApi.getMemoryTree).toHaveBeenCalledTimes(1);
    });

    it('应该支持文件夹的展开和折叠', async () => {
      mockMemoryApi.getMemoryTree.mockResolvedValue({
        entries: [
          { path: 'docs', is_dir: true },
          { path: 'docs/README.md', is_dir: false },
          { path: 'docs/guide.md', is_dir: false },
        ],
      });
      
      renderMemoryTab();
      
      await waitFor(() => {
        expect(screen.getByText('docs')).toBeInTheDocument();
      });
      
      // 初始状态下子文件不可见
      expect(screen.queryByText('README.md')).not.toBeInTheDocument();
      
      // 点击展开文件夹
      fireEvent.click(screen.getByText('docs'));
      
      await waitFor(() => {
        expect(screen.getByText('README.md')).toBeInTheDocument();
        expect(screen.getByText('guide.md')).toBeInTheDocument();
      });
    });
  });

  describe('REQ_MEMORY_002: 文件内容查看', () => {
    it('应该能够选择文件并显示内容', async () => {
      mockMemoryApi.readMemory.mockResolvedValue({
        path: 'USER.md',
        content: '# User Profile\n\nThis is user information.',
        updated_at: null,
      });
      
      renderMemoryTab();
      
      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });
      
      // 选择文件
      fireEvent.click(screen.getByText('USER.md'));
      
      await waitFor(() => {
        expect(mockMemoryApi.readMemory).toHaveBeenCalledWith('USER.md');
        expect(screen.getByText('USER.md')).toBeInTheDocument(); // 文件路径显示
      });
    });

    it('应该支持Markdown文件的渲染', async () => {
      mockMemoryApi.readMemory.mockResolvedValue({
        path: 'notes.md',
        content: '# Title\n\nSome **bold** text.',
        updated_at: null,
      });
      
      renderMemoryTab();
      
      await waitFor(() => {
        fireEvent.click(screen.getByText('notes.md'));
      });
      
      await waitFor(() => {
        expect(screen.getByTestId('markdown')).toBeInTheDocument();
      });
    });
  });

  describe('REQ_MEMORY_003: 文件编辑功能', () => {
    it('应该能够编辑文件内容', async () => {
      mockMemoryApi.readMemory.mockResolvedValue({
        path: 'USER.md',
        content: 'Original content',
        updated_at: null,
      });
      
      mockMemoryApi.writeMemory.mockResolvedValue({
        path: 'USER.md',
        status: 'success',
      });
      
      renderMemoryTab();
      
      await waitFor(() => {
        fireEvent.click(screen.getByText('USER.md'));
      });
      
      await waitFor(() => {
        fireEvent.click(screen.getByText('编辑'));
      });
      
      // 修改内容
      const textareas = screen.getAllByRole('textbox');
      const textarea = textareas.find(el => el.tagName === 'TEXTAREA');
      expect(textarea).toBeInTheDocument();
      fireEvent.change(textarea!, { target: { value: 'Modified content' } });
      
      // 保存
      fireEvent.click(screen.getByText('保存'));
      
      await waitFor(() => {
        expect(mockMemoryApi.writeMemory).toHaveBeenCalledWith('USER.md', 'Modified content');
      });
    });

    it('应该能够取消编辑并恢复原内容', async () => {
      mockMemoryApi.readMemory.mockResolvedValue({
        path: 'USER.md',
        content: 'Original content',
        updated_at: null,
      });
      
      renderMemoryTab();
      
      await waitFor(() => {
        fireEvent.click(screen.getByText('USER.md'));
      });
      
      await waitFor(() => {
        fireEvent.click(screen.getByText('编辑'));
      });
      
      // 修改内容
      const textareas = screen.getAllByRole('textbox');
      const textarea = textareas.find(el => el.tagName === 'TEXTAREA');
      expect(textarea).toBeInTheDocument();
      fireEvent.change(textarea!, { target: { value: 'Modified content' } });
      
      // 取消编辑
      fireEvent.click(screen.getByText('取消'));
      
      // 应该退出编辑模式
      expect(screen.getByText('编辑')).toBeInTheDocument();
      expect(screen.queryByText('保存')).not.toBeInTheDocument();
    });
  });

  describe('REQ_MEMORY_004: 文件删除保护', () => {
    it('应该对系统核心文件隐藏删除按钮', async () => {
      const protectedFiles = ['SOUL.md', 'IDENTITY.md', 'AGENTS.md'];
      
      for (const fileName of protectedFiles) {
        mockMemoryApi.readMemory.mockResolvedValue({
          path: fileName,
          content: 'Protected content',
          updated_at: null,
        });
        
        renderMemoryTab();
        
        await waitFor(() => {
          fireEvent.click(screen.getByText(fileName));
        });
        
        await waitFor(() => {
          expect(screen.getByText('编辑')).toBeInTheDocument();
          expect(screen.queryByText('删除')).not.toBeInTheDocument();
        });
        
        // 清理DOM以便下次测试
        screen.unmount?.();
      }
    });

    it('应该对非保护文件显示删除按钮', async () => {
      const normalFiles = ['USER.md', 'notes.md'];
      
      for (const fileName of normalFiles) {
        mockMemoryApi.readMemory.mockResolvedValue({
          path: fileName,
          content: 'Normal content',
          updated_at: null,
        });
        
        renderMemoryTab();
        
        await waitFor(() => {
          fireEvent.click(screen.getByText(fileName));
        });
        
        await waitFor(() => {
          expect(screen.getByText('编辑')).toBeInTheDocument();
          expect(screen.getByText('删除')).toBeInTheDocument();
        });
        
        // 清理DOM以便下次测试
        screen.unmount?.();
      }
    });
  });

  describe('REQ_MEMORY_005: 文件删除功能', () => {
    it('应该能够删除非保护文件', async () => {
      mockMemoryApi.deleteMemoryLocal.mockResolvedValue({
        success: true,
        message: '文件删除成功',
        is_protected: false,
      });
      
      renderMemoryTab();
      
      await waitFor(() => {
        fireEvent.click(screen.getByText('USER.md'));
      });
      
      await waitFor(() => {
        fireEvent.click(screen.getByText('删除'));
      });
      
      // 确认删除
      await waitFor(() => {
        expect(screen.getByText('删除文件')).toBeInTheDocument();
      });
      
      fireEvent.click(screen.getAllByText('删除')[1]); // 确认按钮
      
      await waitFor(() => {
        expect(mockMemoryApi.deleteMemoryLocal).toHaveBeenCalledWith('USER.md', false);
      });
    });

    it('应该在删除前显示确认对话框', async () => {
      renderMemoryTab();
      
      await waitFor(() => {
        fireEvent.click(screen.getByText('USER.md'));
      });
      
      await waitFor(() => {
        fireEvent.click(screen.getByText('删除'));
      });
      
      // 确认对话框应该出现
      expect(screen.getByText('删除文件')).toBeInTheDocument();
      expect(screen.getByText(/确定要删除文件.*吗？/)).toBeInTheDocument();
      expect(screen.getByText('取消')).toBeInTheDocument();
    });

    it('应该能够取消删除操作', async () => {
      renderMemoryTab();
      
      await waitFor(() => {
        fireEvent.click(screen.getByText('USER.md'));
      });
      
      await waitFor(() => {
        fireEvent.click(screen.getByText('删除'));
      });
      
      // 取消删除
      fireEvent.click(screen.getByText('取消'));
      
      // 对话框应该关闭
      expect(screen.queryByText('删除文件')).not.toBeInTheDocument();
      expect(mockMemoryApi.deleteMemoryLocal).not.toHaveBeenCalled();
    });
  });

  describe('REQ_MEMORY_006: 搜索功能', () => {
    it('应该能够搜索记忆内容', async () => {
      const searchResults = [
        {
          path: 'notes.md',
          content: 'This contains the search term',
          score: 0.95,
        },
        {
          path: 'docs/guide.md',
          content: 'Another file with search term',
          score: 0.80,
        },
      ];
      
      mockMemoryApi.searchMemory.mockResolvedValue(searchResults);
      
      renderMemoryTab();
      
      // 输入搜索查询
      const searchInput = screen.getByPlaceholderText('搜索记忆...');
      fireEvent.change(searchInput, { target: { value: 'search term' } });
      
      await waitFor(() => {
        expect(mockMemoryApi.searchMemory).toHaveBeenCalledWith('search term');
        expect(screen.getByText('notes.md')).toBeInTheDocument();
        expect(screen.getByText('docs/guide.md')).toBeInTheDocument();
        expect(screen.getByText('相关度: 95%')).toBeInTheDocument();
        expect(screen.getByText('相关度: 80%')).toBeInTheDocument();
      });
    });

    it('应该显示搜索结果的内容预览', async () => {
      const searchResults = [
        {
          path: 'notes.md',
          content: 'This is a long content that should be truncated for preview purposes',
          score: 0.95,
        },
      ];
      
      mockMemoryApi.searchMemory.mockResolvedValue(searchResults);
      
      renderMemoryTab();
      
      const searchInput = screen.getByPlaceholderText('搜索记忆...');
      fireEvent.change(searchInput, { target: { value: 'content' } });
      
      await waitFor(() => {
        expect(screen.getByText(/This is a long content that should be truncated/)).toBeInTheDocument();
      });
    });

    it('应该能够从搜索结果中选择文件', async () => {
      const searchResults = [
        {
          path: 'notes.md',
          content: 'Search result content',
          score: 0.95,
        },
      ];
      
      mockMemoryApi.searchMemory.mockResolvedValue(searchResults);
      mockMemoryApi.readMemory.mockResolvedValue({
        path: 'notes.md',
        content: 'Full file content',
        updated_at: null,
      });
      
      renderMemoryTab();
      
      const searchInput = screen.getByPlaceholderText('搜索记忆...');
      fireEvent.change(searchInput, { target: { value: 'content' } });
      
      await waitFor(() => {
        fireEvent.click(screen.getByText('notes.md'));
      });
      
      await waitFor(() => {
        expect(mockMemoryApi.readMemory).toHaveBeenCalledWith('notes.md');
      });
    });
  });

  describe('REQ_MEMORY_007: 已删除文件处理', () => {
    it('应该正确识别已删除的文件', async () => {
      mockMemoryContentUtils.isDeleted.mockReturnValue(true);
      mockMemoryApi.readMemory.mockResolvedValue({
        path: 'deleted.md',
        content: '<!-- DELETED -->',
        updated_at: null,
      });
      
      renderMemoryTab();
      
      await waitFor(() => {
        fireEvent.click(screen.getByText('USER.md'));
      });
      
      await waitFor(() => {
        expect(screen.getByText(/已被删除/)).toBeInTheDocument();
      });
    });

    it('应该对已删除文件显示空内容', async () => {
      mockMemoryContentUtils.isDeleted.mockReturnValue(true);
      mockMemoryContentUtils.getActualContent.mockReturnValue('');
      mockMemoryApi.readMemory.mockResolvedValue({
        path: 'deleted.md',
        content: '<!-- DELETED -->',
        updated_at: null,
      });
      
      renderMemoryTab();
      
      await waitFor(() => {
        fireEvent.click(screen.getByText('USER.md'));
      });
      
      await waitFor(() => {
        expect(mockMemoryContentUtils.getActualContent).toHaveBeenCalled();
      });
    });
  });

  describe('REQ_MEMORY_008: 错误处理', () => {
    it('应该显示统一的错误提示格式', async () => {
      const testCases = [
        {
          operation: 'read',
          error: new Error('File not found'),
          expectedMessage: '读取文件失败: File not found',
        },
        {
          operation: 'write',
          error: new Error('Permission denied'),
          expectedMessage: '保存文件失败: Permission denied',
        },
        {
          operation: 'delete',
          error: new Error('Network error'),
          expectedMessage: '删除文件失败: Network error',
        },
      ];
      
      for (const testCase of testCases) {
        if (testCase.operation === 'read') {
          mockMemoryApi.readMemory.mockRejectedValue(testCase.error);
        } else if (testCase.operation === 'write') {
          mockMemoryApi.writeMemory.mockRejectedValue(testCase.error);
        } else if (testCase.operation === 'delete') {
          mockMemoryApi.deleteMemoryLocal.mockRejectedValue(testCase.error);
        }
        
        renderMemoryTab();
        
        await waitFor(() => {
          fireEvent.click(screen.getByText('USER.md'));
        });
        
        if (testCase.operation === 'read') {
          // 读取错误会在文件选择时触发
          await waitFor(() => {
            expect(screen.getByText(testCase.expectedMessage)).toBeInTheDocument();
          });
        } else if (testCase.operation === 'write') {
          await waitFor(() => {
            fireEvent.click(screen.getByText('编辑'));
            fireEvent.click(screen.getByText('保存'));
          });
          
          await waitFor(() => {
            expect(screen.getByText(testCase.expectedMessage)).toBeInTheDocument();
          });
        } else if (testCase.operation === 'delete') {
          await waitFor(() => {
            fireEvent.click(screen.getByText('删除'));
            fireEvent.click(screen.getAllByText('删除')[1]);
          });
          
          await waitFor(() => {
            expect(screen.getByText(testCase.expectedMessage)).toBeInTheDocument();
          });
        }
        
        // 清理DOM以便下次测试
        screen.unmount?.();
        vi.clearAllMocks();
      }
    });
  });
});