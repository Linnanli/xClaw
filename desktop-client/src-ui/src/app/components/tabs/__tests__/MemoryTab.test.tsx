import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { MemoryTab } from '../MemoryTab';
import { memoryApi } from '../../../utils/tauri';

// Mock the tauri API
vi.mock('../../../utils/tauri', () => ({
  memoryApi: {
    getMemoryTree: vi.fn(),
    readMemory: vi.fn(),
    writeMemory: vi.fn(),
    searchMemory: vi.fn(),
  },
}));

// Mock the theme context
vi.mock('../../../contexts/ThemeContext', () => ({
  useTheme: () => ({ theme: 'dark' }),
}));

describe('MemoryTab', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('Tree Structure Display', () => {
    it('should display memory tree structure', async () => {
      const mockTree = {
        entries: [
          { path: 'folder1', is_dir: true },
          { path: 'file1.md', is_dir: false },
        ],
      };

      vi.mocked(memoryApi.getMemoryTree).mockResolvedValue(mockTree);

      render(<MemoryTab />);

      await waitFor(() => {
        expect(screen.getByText('folder1')).toBeInTheDocument();
        expect(screen.getByText('file1.md')).toBeInTheDocument();
      });
    });

    it('should expand folder when clicked', async () => {
      const mockTree = {
        entries: [
          { path: 'folder1', is_dir: true },
          { path: 'folder1/subfolder', is_dir: true },
          { path: 'folder1/file2.md', is_dir: false },
        ],
      };

      vi.mocked(memoryApi.getMemoryTree).mockResolvedValue(mockTree);

      render(<MemoryTab />);

      await waitFor(() => {
        expect(screen.getByText('folder1')).toBeInTheDocument();
      });

      // Click to expand folder
      fireEvent.click(screen.getByText('folder1'));

      await waitFor(() => {
        expect(screen.getByText('subfolder')).toBeInTheDocument();
        expect(screen.getByText('file2.md')).toBeInTheDocument();
      });
    });
  });

  describe('File Reading', () => {
    it('should read and display file content when clicked', async () => {
      const mockTree = {
        entries: [
          { path: 'test.md', is_dir: false },
        ],
      };

      const mockContent = {
        path: 'test.md',
        content: '# Test Content\n\nThis is a test file.',
      };

      vi.mocked(memoryApi.getMemoryTree).mockResolvedValue(mockTree);
      vi.mocked(memoryApi.readMemory).mockResolvedValue(mockContent);

      render(<MemoryTab />);

      await waitFor(() => {
        expect(screen.getByText('test.md')).toBeInTheDocument();
      });

      // Click to read file
      fireEvent.click(screen.getByText('test.md'));

      await waitFor(() => {
        expect(memoryApi.readMemory).toHaveBeenCalledWith('test.md');
        expect(screen.getByText(/Test Content/)).toBeInTheDocument();
      });
    });

    it('should render markdown files correctly', async () => {
      const mockTree = {
        entries: [
          { path: 'readme.md', is_dir: false },
        ],
      };

      const mockContent = {
        path: 'readme.md',
        content: '# Heading\n\n**Bold text**',
      };

      vi.mocked(memoryApi.getMemoryTree).mockResolvedValue(mockTree);
      vi.mocked(memoryApi.readMemory).mockResolvedValue(mockContent);

      render(<MemoryTab />);

      await waitFor(() => {
        expect(screen.getByText('readme.md')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('readme.md'));

      await waitFor(() => {
        // Check if markdown is rendered (h1 tag)
        const heading = screen.getByRole('heading', { level: 1 });
        expect(heading).toHaveTextContent('Heading');
      });
    });
  });

  describe('File Editing', () => {
    it('should enable edit mode when edit button is clicked', async () => {
      const mockTree = {
        entries: [
          { path: 'edit.md', is_dir: false },
        ],
      };

      const mockContent = {
        path: 'edit.md',
        content: 'Original content',
      };

      vi.mocked(memoryApi.getMemoryTree).mockResolvedValue(mockTree);
      vi.mocked(memoryApi.readMemory).mockResolvedValue(mockContent);

      render(<MemoryTab />);

      await waitFor(() => {
        expect(screen.getByText('edit.md')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('edit.md'));

      await waitFor(() => {
        expect(screen.getByText(/Original content/)).toBeInTheDocument();
      });

      // Click edit button
      const editButton = screen.getByRole('button', { name: /编辑/i });
      fireEvent.click(editButton);

      // Check if textarea is visible (use getAllByRole to get the textarea specifically)
      const textboxes = screen.getAllByRole('textbox');
      const textarea = textboxes.find(el => el.tagName === 'TEXTAREA');
      expect(textarea).toHaveValue('Original content');
    });

    it('should save edited content', async () => {
      const mockTree = {
        entries: [
          { path: 'save.md', is_dir: false },
        ],
      };

      const mockContent = {
        path: 'save.md',
        content: 'Original content',
      };

      const mockWriteResponse = {
        path: 'save.md',
        status: 'written',
      };

      vi.mocked(memoryApi.getMemoryTree).mockResolvedValue(mockTree);
      vi.mocked(memoryApi.readMemory).mockResolvedValue(mockContent);
      vi.mocked(memoryApi.writeMemory).mockResolvedValue(mockWriteResponse);

      render(<MemoryTab />);

      await waitFor(() => {
        expect(screen.getByText('save.md')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('save.md'));

      await waitFor(() => {
        expect(screen.getByText(/Original content/)).toBeInTheDocument();
      });

      // Enter edit mode
      const editButton = screen.getByRole('button', { name: /编辑/i });
      fireEvent.click(editButton);

      // Edit content (use getAllByRole to get the textarea specifically)
      const textboxes = screen.getAllByRole('textbox');
      const textarea = textboxes.find(el => el.tagName === 'TEXTAREA');
      fireEvent.change(textarea!, { target: { value: 'Updated content' } });

      // Save
      const saveButton = screen.getByRole('button', { name: /保存/i });
      fireEvent.click(saveButton);

      await waitFor(() => {
        expect(memoryApi.writeMemory).toHaveBeenCalledWith('save.md', 'Updated content');
      });
    });
  });

  describe('Search', () => {
    it('should search memories when search query is entered', async () => {
      const mockSearchResults = [
        { path: 'result1.md', content: 'Search result 1', score: 0.9 },
        { path: 'result2.md', content: 'Search result 2', score: 0.8 },
      ];

      vi.mocked(memoryApi.searchMemory).mockResolvedValue(mockSearchResults);

      render(<MemoryTab />);

      // Enter search query
      const searchInput = screen.getByPlaceholderText(/搜索/i);
      fireEvent.change(searchInput, { target: { value: 'test query' } });

      await waitFor(() => {
        expect(memoryApi.searchMemory).toHaveBeenCalledWith('test query');
        expect(screen.getByText('result1.md')).toBeInTheDocument();
        expect(screen.getByText('result2.md')).toBeInTheDocument();
      });
    });
  });

  describe('Error Handling', () => {
    it('should display error message when tree loading fails', async () => {
      vi.mocked(memoryApi.getMemoryTree).mockRejectedValue(new Error('Network error'));

      render(<MemoryTab />);

      await waitFor(() => {
        expect(screen.getByText(/Failed to load/i)).toBeInTheDocument();
      });
    });

    it('should display error message when file reading fails', async () => {
      const mockTree = {
        entries: [
          { path: 'error.md', is_dir: false },
        ],
      };

      vi.mocked(memoryApi.getMemoryTree).mockResolvedValue(mockTree);
      vi.mocked(memoryApi.readMemory).mockRejectedValue(new Error('File not found'));

      render(<MemoryTab />);

      await waitFor(() => {
        expect(screen.getByText('error.md')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('error.md'));

      await waitFor(() => {
        expect(screen.getByText(/Failed to read file/i)).toBeInTheDocument();
      });
    });
  });
});
