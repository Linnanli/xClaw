/**
 * MemoryTab 安全审计测试（重写版）
 *
 * 覆盖维度：
 * - 路径遍历攻击防护
 * - XSS 攻击防护
 * - 输入验证和清理
 * - 权限和访问控制
 * - 错误处理安全性
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

describe('MemoryTab - 安全审计测试', () => {
  beforeEach(() => {
    vi.clearAllMocks();

    mockMemoryApi.getMemoryTree.mockResolvedValue({
      entries: [
        { path: 'SOUL.md', is_dir: false },
        { path: 'IDENTITY.md', is_dir: false },
        { path: 'USER.md', is_dir: false },
      ],
    });

    mockMemoryApi.readMemory.mockResolvedValue({
      path: 'USER.md',
      content: 'Test content',
      updated_at: undefined,
    });

    mockMemoryApi.isMemoryFileProtected.mockImplementation(async (path: string) => {
      const name = path.split('/').pop() || '';
      return ['SOUL.md', 'IDENTITY.md', 'AGENTS.md'].includes(name);
    });

    mockContentUtils.isDeleted.mockReturnValue(false);
    mockContentUtils.getActualContent.mockImplementation((c: any) => c.content);
    mockContentUtils.shouldShowInTree.mockReturnValue(true);
  });

  /* ── 路径遍历攻击防护 ── */

  describe('test_security_path_traversal', () => {
    it('应该安全处理包含路径遍历的文件名（多层路径）', async () => {
      // 多层路径遍历文件因缺少父目录节点不会出现在树中
      mockMemoryApi.getMemoryTree.mockResolvedValue({
        entries: [{ path: '../../../etc/passwd', is_dir: false }],
      });

      mockMemoryApi.readMemory.mockResolvedValue({
        path: '../../../etc/passwd',
        content: 'Test',
        updated_at: undefined,
      });

      renderMemoryTab();

      // 关键验证：组件不应崩溃，应正常渲染
      await waitFor(() => {
        expect(screen.getByPlaceholderText('搜索记忆文件...')).toBeInTheDocument();
      });

      // 多层路径遍历文件因缺少父目录节点，树为空
      expect(screen.getByText('暂无记忆条目')).toBeInTheDocument();
    });

    it('应该安全处理反斜杠路径遍历文件', async () => {
      // 反斜杠路径不含 '/'，被视为根级文件名
      mockMemoryApi.getMemoryTree.mockResolvedValue({
        entries: [{ path: '..\\..\\windows\\system32', is_dir: false }],
      });

      mockMemoryApi.readMemory.mockResolvedValue({
        path: '..\\..\\windows\\system32',
        content: 'Test',
        updated_at: undefined,
      });

      renderMemoryTab();

      // 组件不应崩溃，文件名作为根级节点正常显示
      await waitFor(() => {
        expect(screen.getByText('..\\..\\windows\\system32')).toBeInTheDocument();
      });
    });

    it('应该安全处理根级别的路径遍历文件', async () => {
      mockMemoryApi.getMemoryTree.mockResolvedValue({
        entries: [{ path: '..passwd', is_dir: false }],
      });

      mockMemoryApi.readMemory.mockResolvedValue({
        path: '..passwd',
        content: 'Test',
        updated_at: undefined,
      });

      renderMemoryTab();

      await waitFor(() => {
        expect(screen.getByText('..passwd')).toBeInTheDocument();
      });
    });
  });

  /* ── XSS 攻击防护 ── */

  describe('test_security_xss_prevention', () => {
    it('应该安全渲染包含 XSS payload 的文件名', async () => {
      // 使用不含 HTML 标签的 XSS payload（避免 jsdom 解析问题）
      const xssName = 'alert("xss")&lt;script&gt;.md';
      mockMemoryApi.getMemoryTree.mockResolvedValue({
        entries: [{ path: xssName, is_dir: false }],
      });

      mockMemoryApi.readMemory.mockResolvedValue({
        path: xssName,
        content: 'Test',
        updated_at: undefined,
      });

      renderMemoryTab();

      await waitFor(() => {
        const el = screen.getByText(xssName);
        expect(el).toBeInTheDocument();
        // React 自动转义，应该是文本节点
        expect(el.tagName).toBe('SPAN');
      });
    });

    it('应该安全渲染包含 HTML 标签的文件名', async () => {
      // 包含真实 HTML 标签的文件名
      const htmlName = '<img onerror=alert(1)>.md';
      mockMemoryApi.getMemoryTree.mockResolvedValue({
        entries: [{ path: htmlName, is_dir: false }],
      });

      mockMemoryApi.readMemory.mockResolvedValue({
        path: htmlName,
        content: 'Test',
        updated_at: undefined,
      });

      const { container } = renderMemoryTab();

      await waitFor(() => {
        // React 会将 HTML 标签转义为文本，不会执行
        // 使用 container 查询确认没有真实的 img 元素被创建
        const imgs = container.querySelectorAll('img');
        expect(imgs.length).toBe(0);
        // 组件不应崩溃
        expect(screen.getByPlaceholderText('搜索记忆文件...')).toBeInTheDocument();
      });
    });

    it('应该安全渲染包含 XSS payload 的文件内容', async () => {
      const xssContent = '<script>alert("xss")</script><img src=x onerror=alert(1)>';
      mockMemoryApi.readMemory.mockResolvedValue({
        path: 'USER.md',
        content: xssContent,
        updated_at: undefined,
      });

      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('USER.md'));

      await waitFor(() => {
        // ReactMarkdown 或 React 会安全渲染
        expect(screen.getByTestId('markdown')).toBeInTheDocument();
      });
    });
  });

  /* ── 输入验证 ── */

  describe('test_security_input_validation', () => {
    it('应该安全处理超长搜索输入', async () => {
      const longQuery = 'a'.repeat(10000);
      renderMemoryTab();

      const input = screen.getByPlaceholderText('搜索记忆文件...');
      fireEvent.change(input, { target: { value: longQuery } });

      // 不应崩溃
      expect(input).toHaveValue(longQuery);
    });

    it('应该安全处理特殊字符搜索', async () => {
      const specialQueries = [
        '<script>alert(1)</script>',
        '../../etc/passwd',
        'SELECT * FROM users',
        '${process.env}',
      ];

      renderMemoryTab();

      for (const query of specialQueries) {
        const input = screen.getByPlaceholderText('搜索记忆文件...');
        fireEvent.change(input, { target: { value: query } });
        // 不应崩溃
        expect(input).toHaveValue(query);
      }
    });

    it('应该安全处理恶意编辑内容', async () => {
      const maliciousContent = '<script>alert("xss")</script>';
      mockMemoryApi.writeMemory.mockResolvedValue({ path: 'USER.md', status: 'success' });

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
      fireEvent.change(textarea, { target: { value: maliciousContent } });
      fireEvent.click(screen.getByText('保存'));

      await waitFor(() => {
        // 内容原样传递给后端（后端负责安全处理）
        expect(mockMemoryApi.writeMemory).toHaveBeenCalledWith('USER.md', maliciousContent);
      });
    });
  });

  /* ── 权限和访问控制 ── */

  describe('test_security_access_control', () => {
    it('受保护文件不应有删除按钮', async () => {
      const protectedFiles = ['SOUL.md', 'IDENTITY.md'];

      for (const fileName of protectedFiles) {
        mockMemoryApi.readMemory.mockResolvedValue({
          path: fileName,
          content: 'Protected',
          updated_at: undefined,
        });

        const { unmount } = renderMemoryTab();

        await waitFor(() => {
          expect(screen.getByText(fileName)).toBeInTheDocument();
        });

        fireEvent.click(screen.getByText(fileName));

        await waitFor(() => {
          expect(screen.getByText('编辑')).toBeInTheDocument();
          expect(screen.queryByText('删除')).not.toBeInTheDocument();
        });

        unmount();
      }
    });

    it('非保护文件应有删除按钮', async () => {
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

  /* ── 错误处理安全性 ── */

  describe('test_security_error_handling', () => {
    it('API 错误不应导致组件崩溃', async () => {
      mockMemoryApi.readMemory.mockRejectedValue(
        new Error('Database connection failed: password=secret123'),
      );

      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('USER.md'));

      await waitFor(() => {
        // 错误应该被捕获并显示
        expect(screen.getByText(/读取文件失败/)).toBeInTheDocument();
      });
    });

    it('删除 API 异常不应导致组件崩溃', async () => {
      mockMemoryApi.deleteMemoryLocal.mockRejectedValue(new Error('Unexpected'));

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
        expect(screen.getByText(/删除失败/)).toBeInTheDocument();
      });
    });
  });

  /* ── 资源安全 ── */

  describe('test_security_resource_safety', () => {
    it('应该安全处理大文件内容', async () => {
      const largeContent = 'A'.repeat(1_000_000);
      mockMemoryApi.readMemory.mockResolvedValue({
        path: 'USER.md',
        content: largeContent,
        updated_at: undefined,
      });

      renderMemoryTab();
      await waitFor(() => {
        expect(screen.getByText('USER.md')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('USER.md'));

      await waitFor(() => {
        expect(mockMemoryApi.readMemory).toHaveBeenCalled();
      });
    });
  });
});
