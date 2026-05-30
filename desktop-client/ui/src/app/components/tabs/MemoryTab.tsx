/**
 * MemoryTab - 记忆管理面板
 *
 * 设计稿：搜索栏 + 左侧200px文件树 + 右侧预览面板
 * 文件树：文件夹展开/折叠、文件选中高亮、系统文件分区
 * 预览面板：文件路径 + 编辑/删除按钮 + Markdown渲染 + 更新时间
 */

import { useState, useEffect, useCallback } from 'react';
import {
  Search,
  FolderOpen,
  Folder,
  FileText,
  ChevronDown,
  ChevronRight,
  Pencil,
  Trash2,
  Shield,
  Timer,
  Save,
  X,
} from 'lucide-react';
import { cn } from '../ui/utils';
import {
  memoryApi,
  memoryContentUtils,
  type TreeEntry,
} from '../../utils/tauri';
import { DeleteConfirmDialog } from '../common/DeleteConfirmDialog';
import ReactMarkdown from 'react-markdown';

/* ── 类型定义 ── */

interface TreeNode {
  name: string;
  path: string;
  is_dir: boolean;
  children?: TreeNode[];
  expanded?: boolean;
}

/* ── 主组件 ── */

export function MemoryTab() {
  const [searchQuery, setSearchQuery] = useState('');
  const [treeNodes, setTreeNodes] = useState<TreeNode[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // 文件预览状态
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [fileContent, setFileContent] = useState('');
  const [fileUpdatedAt, setFileUpdatedAt] = useState<string | null>(null);
  const [isEditing, setIsEditing] = useState(false);
  const [editContent, setEditContent] = useState('');
  const [isProtectedFile, setIsProtectedFile] = useState(false);

  // 删除确认
  const [deleteDialog, setDeleteDialog] = useState<{
    isOpen: boolean;
    filePath: string | null;
  }>({ isOpen: false, filePath: null });
  const [opLoading, setOpLoading] = useState(false);

  /* ── 数据加载 ── */

  const buildTree = useCallback((entries: TreeEntry[]): TreeNode[] => {
    const nodes: TreeNode[] = [];
    const pathMap = new Map<string, TreeNode>();
    const sorted = [...entries].sort((a, b) => a.path.localeCompare(b.path));

    for (const entry of sorted) {
      const parts = entry.path.split('/');
      const node: TreeNode = {
        name: parts[parts.length - 1],
        path: entry.path,
        is_dir: entry.is_dir,
        children: entry.is_dir ? [] : undefined,
        expanded: false,
      };
      pathMap.set(entry.path, node);

      if (parts.length === 1) {
        nodes.push(node);
      } else {
        const parentPath = parts.slice(0, -1).join('/');
        const parent = pathMap.get(parentPath);
        parent?.children?.push(node);
      }
    }
    return nodes;
  }, []);

  const filterDeleted = useCallback(
    async (nodes: TreeNode[]): Promise<TreeNode[]> => {
      const result: TreeNode[] = [];
      for (const node of nodes) {
        if (node.is_dir) {
          const children = node.children
            ? await filterDeleted(node.children)
            : undefined;
          if (!children || children.length > 0 || node.path.split('/').length === 1) {
            result.push({ ...node, children });
          }
        } else {
          try {
            const content = await memoryApi.readMemory(node.path);
            if (!memoryContentUtils.isDeleted(content)) {
              result.push(node);
            }
          } catch {
            result.push(node);
          }
        }
      }
      return result;
    },
    [],
  );

  const loadTree = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const tree = await memoryApi.getMemoryTree();
      const nodes = buildTree(tree.entries);
      const filtered = await filterDeleted(nodes);
      setTreeNodes(filtered);
    } catch (err) {
      console.error('Failed to load memory tree:', err);
      setError('加载记忆失败');
      setTreeNodes([]);
    } finally {
      setLoading(false);
    }
  }, [buildTree, filterDeleted]);

  useEffect(() => {
    loadTree();
  }, [loadTree]);

  /* ── 文件树操作 ── */

  const toggleFolder = (path: string) => {
    const update = (nodes: TreeNode[]): TreeNode[] =>
      nodes.map((n) =>
        n.path === path
          ? { ...n, expanded: !n.expanded }
          : n.children
            ? { ...n, children: update(n.children) }
            : n,
      );
    setTreeNodes(update(treeNodes));
  };

  const handleFileClick = async (path: string) => {
    try {
      setSelectedFile(path);
      setIsEditing(false);
      setError(null);

      const isProtected = await memoryApi.isMemoryFileProtected(path);
      setIsProtectedFile(isProtected);

      const content = await memoryApi.readMemory(path);
      if (memoryContentUtils.isDeleted(content)) {
        setFileContent('');
        setEditContent('');
        setError(`文件 ${path} 已被删除`);
        return;
      }

      const actual = memoryContentUtils.getActualContent(content);
      setFileContent(actual);
      setEditContent(actual);
      setFileUpdatedAt(content.updated_at || null);
    } catch (err) {
      console.error('Failed to read file:', err);
      setError(`读取文件失败: ${err instanceof Error ? err.message : '未知错误'}`);
    }
  };

  /* ── 编辑操作 ── */

  const handleSave = async () => {
    if (!selectedFile) return;
    try {
      await memoryApi.writeMemory(selectedFile, editContent);
      setFileContent(editContent);
      setIsEditing(false);
      setError(null);
    } catch (err) {
      setError(`保存失败: ${err instanceof Error ? err.message : '未知错误'}`);
    }
  };

  /* ── 删除操作 ── */

  const handleConfirmDelete = async () => {
    if (!deleteDialog.filePath) return;
    try {
      setOpLoading(true);
      const result = await memoryApi.deleteMemoryLocal(deleteDialog.filePath, false);
      if (result.success) {
        if (selectedFile === deleteDialog.filePath) {
          setSelectedFile(null);
          setFileContent('');
          setEditContent('');
          setIsEditing(false);
          setIsProtectedFile(false);
        }
        await loadTree();
      } else {
        setError(result.message);
      }
      setDeleteDialog({ isOpen: false, filePath: null });
    } catch (err) {
      setError(`删除失败: ${err instanceof Error ? err.message : '未知错误'}`);
    } finally {
      setOpLoading(false);
    }
  };

  /* ── 搜索过滤 ── */

  const filterBySearch = useCallback(
    (nodes: TreeNode[]): TreeNode[] => {
      if (!searchQuery.trim()) return nodes;
      const q = searchQuery.toLowerCase();
      const result: TreeNode[] = [];
      for (const node of nodes) {
        if (node.is_dir) {
          const children = node.children ? filterBySearch(node.children) : [];
          if (children.length > 0) {
            result.push({ ...node, children, expanded: true });
          }
        } else if (node.name.toLowerCase().includes(q)) {
          result.push(node);
        }
      }
      return result;
    },
    [searchQuery],
  );

  const displayNodes = filterBySearch(treeNodes);

  /* ── 判断系统文件 ── */

  const isSystemFile = (name: string) =>
    ['SOUL.md', 'IDENTITY.md'].includes(name);

  /* ── 渲染文件树节点 ── */

  const renderNode = (node: TreeNode, depth: number = 0) => {
    const isSelected = selectedFile === node.path;
    const isSys = isSystemFile(node.name);

    if (node.is_dir) {
      return (
        <div key={node.path}>
          <button
            onClick={() => toggleFolder(node.path)}
            className={cn(
              'flex w-full items-center gap-1.5 rounded-md px-2 py-1.5 text-[13px] font-medium transition-colors hover:bg-accent',
            )}
            style={{ paddingLeft: `${depth * 16 + 8}px` }}
          >
            {node.expanded ? (
              <ChevronDown className="size-3 text-muted-foreground" />
            ) : (
              <ChevronRight className="size-3 text-muted-foreground" />
            )}
            {node.expanded ? (
              <FolderOpen className="size-3.5 text-primary" />
            ) : (
              <Folder className="size-3.5 text-muted-foreground" />
            )}
            <span className={cn(node.expanded ? 'text-foreground' : 'text-muted-foreground')}>
              {node.name}
            </span>
          </button>
          {node.expanded &&
            node.children?.map((child) => renderNode(child, depth + 1))}
        </div>
      );
    }

    return (
      <button
        key={node.path}
        onClick={() => handleFileClick(node.path)}
        className={cn(
          'flex w-full items-center gap-1.5 rounded-md px-2 py-1.5 text-xs transition-colors',
          isSelected
            ? 'bg-primary/10 font-medium text-primary'
            : 'text-muted-foreground hover:bg-accent',
        )}
        style={{ paddingLeft: `${depth * 16 + 24}px` }}
      >
        {isSys ? (
          <Shield className="size-[13px] text-[#C8A84B]" />
        ) : (
          <FileText
            className={cn(
              'size-[13px]',
              isSelected ? 'text-primary' : 'text-muted-foreground',
            )}
          />
        )}
        <span>{node.name}</span>
      </button>
    );
  };

  /* ── 分离系统文件 ── */

  const { userNodes, systemNodes } = (() => {
    const user: TreeNode[] = [];
    const sys: TreeNode[] = [];
    for (const node of displayNodes) {
      if (!node.is_dir && isSystemFile(node.name)) {
        sys.push(node);
      } else {
        user.push(node);
      }
    }
    return { userNodes: user, systemNodes: sys };
  })();

  return (
    <div className="flex flex-col gap-4">
      {/* 搜索栏 */}
      <div className="relative">
        <Search className="absolute left-3 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
        <input
          type="text"
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          placeholder="搜索记忆文件..."
          className="h-9 w-full rounded-lg border border-border bg-secondary/30 pl-9 pr-3 text-[13px] text-foreground placeholder:text-muted-foreground/60 focus:outline-none focus:ring-1 focus:ring-primary/30"
        />
      </div>

      {error && (
        <div className="rounded-lg border border-destructive/30 bg-destructive/5 px-3 py-2 text-xs text-destructive">
          {error}
        </div>
      )}

      {/* 主区域：文件树 + 预览 */}
      <div className="flex min-h-[460px] overflow-hidden">
        {/* 左侧文件树 */}
        <div className="w-[200px] shrink-0 overflow-y-auto border-r border-border py-1 pr-0">
          {loading ? (
            <div className="py-8 text-center text-xs text-muted-foreground">
              加载中...
            </div>
          ) : (
            <>
              {userNodes.map((node) => renderNode(node))}
              {systemNodes.length > 0 && (
                <>
                  <div className="mx-2 my-2 h-px bg-border" />
                  <div className="px-2 py-1 text-[11px] font-medium text-muted-foreground/60">
                    系统文件
                  </div>
                  {systemNodes.map((node) => renderNode(node))}
                </>
              )}
              {displayNodes.length === 0 && (
                <div className="py-8 text-center text-xs text-muted-foreground">
                  {searchQuery ? '未找到匹配文件' : '暂无记忆条目'}
                </div>
              )}
            </>
          )}
        </div>

        {/* 右侧预览面板 */}
        <div className="flex flex-1 flex-col pl-4">
          {selectedFile ? (
            <>
              {/* 预览头部 */}
              <div className="flex items-center justify-between pb-2.5">
                <div className="flex items-center gap-2">
                  <FileText className="size-3.5 text-primary" />
                  <span className="text-[13px] font-semibold text-foreground">
                    {selectedFile}
                  </span>
                </div>
                <div className="flex items-center gap-2">
                  {isEditing ? (
                    <>
                      <button
                        onClick={handleSave}
                        className="flex items-center gap-1 rounded-md border border-primary/30 px-2.5 py-1 text-xs font-medium text-primary transition-colors hover:bg-primary/5"
                      >
                        <Save className="size-3" />
                        保存
                      </button>
                      <button
                        onClick={() => {
                          setIsEditing(false);
                          setEditContent(fileContent);
                        }}
                        className="flex items-center gap-1 rounded-md border border-border px-2.5 py-1 text-xs font-medium text-muted-foreground transition-colors hover:bg-accent"
                      >
                        <X className="size-3" />
                        取消
                      </button>
                    </>
                  ) : (
                    <>
                      <button
                        onClick={() => setIsEditing(true)}
                        className="flex items-center gap-1 rounded-md border border-border px-2.5 py-1 text-xs font-medium text-muted-foreground transition-colors hover:bg-accent"
                      >
                        <Pencil className="size-3" />
                        编辑
                      </button>
                      {!isProtectedFile && (
                        <button
                          onClick={() =>
                            setDeleteDialog({
                              isOpen: true,
                              filePath: selectedFile,
                            })
                          }
                          className="flex items-center gap-1 rounded-md border border-destructive/30 px-2.5 py-1 text-xs font-medium text-destructive transition-colors hover:bg-destructive/5"
                        >
                          <Trash2 className="size-3" />
                          删除
                        </button>
                      )}
                    </>
                  )}
                </div>
              </div>

              {/* 分割线 */}
              <div className="h-px bg-border" />

              {/* 内容区 */}
              <div className="flex-1 overflow-y-auto pt-4">
                {isEditing ? (
                  <textarea
                    value={editContent}
                    onChange={(e) => setEditContent(e.target.value)}
                    className="h-full min-h-[360px] w-full resize-none rounded-lg border border-border bg-secondary/20 p-3 font-mono text-sm text-foreground focus:outline-none focus:ring-1 focus:ring-primary/30"
                  />
                ) : (
                  <div className="text-foreground">
                    {selectedFile.endsWith('.md') ? (
                      <div className="prose prose-sm max-w-none dark:prose-invert">
                        <ReactMarkdown>{fileContent}</ReactMarkdown>
                      </div>
                    ) : (
                      <pre className="whitespace-pre-wrap font-mono text-sm">
                        {fileContent}
                      </pre>
                    )}
                  </div>
                )}

                {/* 更新时间 */}
                {fileUpdatedAt && !isEditing && (
                  <div className="mt-4 flex items-center gap-1.5 text-[11px] text-muted-foreground/60">
                    <Timer className="size-3" />
                    最后更新：{new Date(fileUpdatedAt).toLocaleString('zh-CN')}
                  </div>
                )}
              </div>
            </>
          ) : (
            <div className="flex flex-1 items-center justify-center text-sm text-muted-foreground">
              <div className="text-center">
                <FileText className="mx-auto mb-3 size-10 opacity-30" />
                <p>选择一个文件查看内容</p>
              </div>
            </div>
          )}
        </div>
      </div>

      {/* 删除确认 */}
      <DeleteConfirmDialog
        isOpen={deleteDialog.isOpen}
        title="删除文件"
        message={`确定要删除文件 "${deleteDialog.filePath}" 吗？此操作无法撤销。`}
        confirmText="删除"
        cancelText="取消"
        loading={opLoading}
        onConfirm={handleConfirmDelete}
        onCancel={() => setDeleteDialog({ isOpen: false, filePath: null })}
      />
    </div>
  );
}
