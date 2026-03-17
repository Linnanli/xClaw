import { useState, useEffect } from 'react';
import { Search, FileText, FolderTree, Folder, Edit2, Save, X, ChevronRight, ChevronDown, Trash2 } from 'lucide-react';
import { useTheme } from '../../contexts/ThemeContext';
import { memoryApi, memoryContentUtils, TreeEntry, SearchHit, MemoryContent } from '../../utils/tauri';
import { DeleteConfirmDialog } from '../common/DeleteConfirmDialog';
import ReactMarkdown from 'react-markdown';

interface TreeNode {
  name: string;
  path: string;
  is_dir: boolean;
  children?: TreeNode[];
  expanded?: boolean;
  loaded?: boolean;
}

export function MemoryTab() {
  const { theme } = useTheme();
  const [searchQuery, setSearchQuery] = useState('');
  const [treeNodes, setTreeNodes] = useState<TreeNode[]>([]);
  const [searchResults, setSearchResults] = useState<SearchHit[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [isSearching, setIsSearching] = useState(false);
  
  // File viewer state
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [fileContent, setFileContent] = useState<string>('');
  const [isEditing, setIsEditing] = useState(false);
  const [editContent, setEditContent] = useState('');
  const [isProtectedFile, setIsProtectedFile] = useState(false);
  
  // Delete confirmation state
  const [deleteConfirmDialog, setDeleteConfirmDialog] = useState<{
    isOpen: boolean;
    filePath: string | null;
  }>({
    isOpen: false,
    filePath: null,
  });
  const [operationLoading, setOperationLoading] = useState(false);

  useEffect(() => {
    const fetchMemories = async () => {
      try {
        setLoading(true);
        if (searchQuery.trim()) {
          setIsSearching(true);
          const results = await memoryApi.searchMemory(searchQuery);
          setSearchResults(results);
        } else {
          setIsSearching(false);
          const tree = await memoryApi.getMemoryTree();
          const nodes = buildTreeFromEntries(tree.entries);
          setTreeNodes(nodes);
        }
        setError(null);
      } catch (err) {
        console.error('Failed to fetch memories:', err);
        setError('Failed to load memories');
        setTreeNodes([]);
        setSearchResults([]);
      } finally {
        setLoading(false);
      }
    };

    fetchMemories();
  }, [searchQuery]);

  const buildTreeFromEntries = (entries: TreeEntry[]): TreeNode[] => {
    const nodes: TreeNode[] = [];
    const pathMap = new Map<string, TreeNode>();

    // Sort entries to ensure parents come before children
    const sortedEntries = [...entries].sort((a, b) => a.path.localeCompare(b.path));

    for (const entry of sortedEntries) {
      const parts = entry.path.split('/');
      const name = parts[parts.length - 1];
      
      const node: TreeNode = {
        name,
        path: entry.path,
        is_dir: entry.is_dir,
        children: entry.is_dir ? [] : undefined,
        expanded: false,
        loaded: false,
      };

      pathMap.set(entry.path, node);

      if (parts.length === 1) {
        // Top-level entry
        nodes.push(node);
      } else {
        // Find parent
        const parentPath = parts.slice(0, -1).join('/');
        const parent = pathMap.get(parentPath);
        if (parent && parent.children) {
          parent.children.push(node);
        }
      }
    }

    return nodes;
  };

  const toggleFolder = (node: TreeNode) => {
    const updateNode = (nodes: TreeNode[]): TreeNode[] => {
      return nodes.map(n => {
        if (n.path === node.path) {
          return { ...n, expanded: !n.expanded };
        }
        if (n.children) {
          return { ...n, children: updateNode(n.children) };
        }
        return n;
      });
    };

    setTreeNodes(updateNode(treeNodes));
  };

  const handleFileClick = async (path: string) => {
    try {
      setSelectedFile(path);
      setIsEditing(false);
      
      // 检查文件是否受保护
      const isProtected = await memoryApi.isMemoryFileProtected(path);
      setIsProtectedFile(isProtected);
      
      const content = await memoryApi.readMemory(path);
      
      // 检查文件是否已被删除
      if (memoryContentUtils.isDeleted(content)) {
        setFileContent('');
        setEditContent('');
        setError(`文件 ${path} 已被删除`);
        return;
      }
      
      const actualContent = memoryContentUtils.getActualContent(content);
      setFileContent(actualContent);
      setEditContent(actualContent);
      setError(null);
    } catch (err) {
      console.error('Failed to read file:', err);
      const errorMessage = err instanceof Error ? err.message : '未知错误';
      setError(`读取文件失败: ${errorMessage}`);
    }
  };

  const handleEdit = () => {
    setIsEditing(true);
  };

  const handleCancelEdit = () => {
    setIsEditing(false);
    setEditContent(fileContent);
  };

  const handleSave = async () => {
    if (!selectedFile) return;
    
    try {
      await memoryApi.writeMemory(selectedFile, editContent);
      setFileContent(editContent);
      setIsEditing(false);
      setError(null);
    } catch (err) {
      console.error('Failed to save file:', err);
      const errorMessage = err instanceof Error ? err.message : '未知错误';
      setError(`保存文件失败: ${errorMessage}`);
    }
  };

  const handleDelete = () => {
    if (!selectedFile) return;
    setDeleteConfirmDialog({
      isOpen: true,
      filePath: selectedFile,
    });
  };

  const handleConfirmDelete = async () => {
    if (!deleteConfirmDialog.filePath) return;

    try {
      setOperationLoading(true);
      
      // 执行删除（不需要再次检查保护状态，因为受保护文件不会显示删除按钮）
      const result = await memoryApi.deleteMemoryLocal(deleteConfirmDialog.filePath, false);
      
      if (result.success) {
        // 如果删除的是当前选中的文件，清空选择
        if (selectedFile === deleteConfirmDialog.filePath) {
          setSelectedFile(null);
          setFileContent('');
          setEditContent('');
          setIsEditing(false);
          setIsProtectedFile(false);
        }
        
        // 刷新文件树
        if (searchQuery.trim()) {
          const results = await memoryApi.searchMemory(searchQuery);
          setSearchResults(results);
        } else {
          const tree = await memoryApi.getMemoryTree();
          const nodes = buildTreeFromEntries(tree.entries);
          setTreeNodes(nodes);
        }
        
        console.log('✅ 文件删除成功:', result.message);
        setError(null);
      } else {
        setError(result.message);
      }
      
      setDeleteConfirmDialog({ isOpen: false, filePath: null });
    } catch (err) {
      console.error('❌ 删除文件失败:', err);
      const errorMessage = err instanceof Error ? err.message : '未知错误';
      setError(`删除文件失败: ${errorMessage}`);
    } finally {
      setOperationLoading(false);
    }
  };

  const handleCancelDelete = () => {
    setDeleteConfirmDialog({ isOpen: false, filePath: null });
  };

  const renderTreeNode = (node: TreeNode, depth: number = 0): JSX.Element => {
    const paddingLeft = depth * 16 + 8;

    if (node.is_dir) {
      return (
        <div key={node.path}>
          <div
            className={`flex items-center gap-2 p-2 cursor-pointer hover:bg-opacity-10 hover:bg-white rounded ${
              theme === 'dark' ? 'text-white' : 'text-[#333]'
            }`}
            style={{ paddingLeft: `${paddingLeft}px` }}
            onClick={() => toggleFolder(node)}
          >
            {node.expanded ? (
              <ChevronDown size={16} className={theme === 'dark' ? 'text-[#5ddad5]' : 'text-[#667eea]'} />
            ) : (
              <ChevronRight size={16} className={theme === 'dark' ? 'text-[#5ddad5]' : 'text-[#667eea]'} />
            )}
            <Folder size={16} className={theme === 'dark' ? 'text-[#5ddad5]' : 'text-[#667eea]'} />
            <span className="text-sm">{node.name}</span>
          </div>
          {node.expanded && node.children && (
            <div>
              {node.children.map(child => renderTreeNode(child, depth + 1))}
            </div>
          )}
        </div>
      );
    } else {
      return (
        <div
          key={node.path}
          className={`flex items-center gap-2 p-2 cursor-pointer hover:bg-opacity-10 hover:bg-white rounded ${
            theme === 'dark' ? 'text-white' : 'text-[#333]'
          } ${selectedFile === node.path ? (theme === 'dark' ? 'bg-[#1a2942]' : 'bg-[#f0f0f0]') : ''}`}
          style={{ paddingLeft: `${paddingLeft + 16}px` }}
          onClick={() => handleFileClick(node.path)}
        >
          <FileText size={16} className={theme === 'dark' ? 'text-[#5ddad5]' : 'text-[#667eea]'} />
          <span className="text-sm">{node.name}</span>
        </div>
      );
    }
  };

  return (
    <div className="h-full flex">
      {/* Left sidebar - Tree view */}
      <div className={`w-64 border-r ${theme === 'dark' ? 'border-[#1a2942]' : 'border-[#ddd]'} p-4 overflow-y-auto`}>
        <div className="mb-4">
          <div className="relative">
            <Search className={`absolute left-3 top-1/2 -translate-y-1/2 ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`} size={16} />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="搜索记忆..."
              className={`w-full pl-9 pr-3 py-2 text-sm border rounded-lg focus:outline-none ${
                theme === 'dark'
                  ? 'bg-[#0f1d35] border-[#1a2942] focus:border-[#5ddad5] text-white placeholder-gray-500'
                  : 'bg-white border-[#ddd] focus:border-[#667eea] focus:ring-2 focus:ring-[#667eea]/20 text-[#333] placeholder-gray-400'
              }`}
            />
          </div>
        </div>

        {error && (
          <div className="mb-4 p-2 bg-red-400/10 border border-red-400/30 rounded text-red-400 text-xs">
            {error}
          </div>
        )}

        {isSearching ? (
          <div className="space-y-2">
            {searchResults.map((result, index) => (
              <div
                key={index}
                className={`p-2 border rounded cursor-pointer text-sm ${
                  theme === 'dark'
                    ? 'bg-[#0f1d35] border-[#1a2942] hover:border-[#5ddad5]/30'
                    : 'bg-white border-[#ddd] hover:border-[#667eea]/50'
                }`}
                onClick={() => handleFileClick(result.path)}
              >
                <div className={`font-medium mb-1 ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>
                  {result.path}
                </div>
                <div className={`text-xs ${theme === 'dark' ? 'text-gray-400' : 'text-[#666]'}`}>
                  {result.content.substring(0, 100)}...
                </div>
                <div className={`text-xs mt-1 ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>
                  相关度: {(result.score * 100).toFixed(0)}%
                </div>
              </div>
            ))}
            {searchResults.length === 0 && !loading && (
              <div className={`text-center py-8 text-sm ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>
                未找到匹配的记忆
              </div>
            )}
          </div>
        ) : (
          <div>
            {treeNodes.map(node => renderTreeNode(node))}
            {treeNodes.length === 0 && !loading && (
              <div className={`text-center py-8 text-sm ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>
                暂无记忆条目
              </div>
            )}
          </div>
        )}
      </div>

      {/* Right panel - File viewer/editor */}
      <div className="flex-1 flex flex-col">
        {selectedFile ? (
          <>
            {/* File header */}
            <div className={`flex items-center justify-between p-4 border-b ${
              theme === 'dark' ? 'border-[#1a2942]' : 'border-[#ddd]'
            }`}>
              <div className={`font-medium ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>
                {selectedFile}
              </div>
              <div className="flex gap-2">
                {isEditing ? (
                  <>
                    <button
                      onClick={handleSave}
                      className={`px-3 py-1 rounded flex items-center gap-1 text-sm ${
                        theme === 'dark'
                          ? 'bg-[#5ddad5] text-[#0a1628] hover:opacity-90'
                          : 'bg-[#667eea] text-white hover:opacity-90'
                      }`}
                    >
                      <Save size={14} />
                      保存
                    </button>
                    <button
                      onClick={handleCancelEdit}
                      className={`px-3 py-1 rounded flex items-center gap-1 text-sm ${
                        theme === 'dark'
                          ? 'bg-[#1a2942] text-white hover:bg-[#243550]'
                          : 'bg-[#ddd] text-[#333] hover:bg-[#ccc]'
                      }`}
                    >
                      <X size={14} />
                      取消
                    </button>
                  </>
                ) : (
                  <>
                    <button
                      onClick={handleEdit}
                      className={`px-3 py-1 rounded flex items-center gap-1 text-sm ${
                        theme === 'dark'
                          ? 'bg-[#1a2942] text-white hover:bg-[#243550]'
                          : 'bg-[#ddd] text-[#333] hover:bg-[#ccc]'
                      }`}
                    >
                      <Edit2 size={14} />
                      编辑
                    </button>
                    {!isProtectedFile && (
                      <button
                        onClick={handleDelete}
                        className={`px-3 py-1 rounded flex items-center gap-1 text-sm ${
                          theme === 'dark'
                            ? 'bg-red-600 text-white hover:bg-red-700'
                            : 'bg-red-500 text-white hover:bg-red-600'
                        }`}
                      >
                        <Trash2 size={14} />
                        删除
                      </button>
                    )}
                  </>
                )}
              </div>
            </div>

            {/* File content */}
            <div className="flex-1 overflow-y-auto p-6">
              {isEditing ? (
                <textarea
                  value={editContent}
                  onChange={(e) => setEditContent(e.target.value)}
                  className={`w-full h-full p-4 font-mono text-sm border rounded resize-none focus:outline-none ${
                    theme === 'dark'
                      ? 'bg-[#0f1d35] border-[#1a2942] text-white'
                      : 'bg-white border-[#ddd] text-[#333]'
                  }`}
                />
              ) : (
                <div className={theme === 'dark' ? 'text-white' : 'text-[#333]'}>
                  {selectedFile.endsWith('.md') ? (
                    <div className="prose prose-sm max-w-none dark:prose-invert">
                      <ReactMarkdown>{fileContent}</ReactMarkdown>
                    </div>
                  ) : (
                    <pre className="whitespace-pre-wrap font-mono text-sm">{fileContent}</pre>
                  )}
                </div>
              )}
            </div>
          </>
        ) : (
          <div className={`flex-1 flex items-center justify-center ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>
            <div className="text-center">
              <FileText size={48} className="mx-auto mb-4 opacity-50" />
              <p>选择一个文件查看内容</p>
            </div>
          </div>
        )}
      </div>

      {/* 删除确认对话框 */}
      <DeleteConfirmDialog
        isOpen={deleteConfirmDialog.isOpen}
        title="删除文件"
        message={`确定要删除文件 "${deleteConfirmDialog.filePath}" 吗？此操作无法撤销。`}
        confirmText="删除"
        cancelText="取消"
        loading={operationLoading}
        onConfirm={handleConfirmDelete}
        onCancel={handleCancelDelete}
      />
    </div>
  );
}
