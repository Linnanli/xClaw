/**
 * CustomModelModal - 自定义模型管理弹窗
 *
 * Pencil 设计稿 YPJKU 规范：
 * - 弹窗：600x652, cornerRadius:16, shadow blur:80
 * - Header：height:60, 标题"自定义模型" fontSize:16 fontWeight:700
 *   - CPU 图标 fill:#3D8A5A 18x18
 *   - 关闭按钮：cornerRadius:8, fill:#F5F4F2, 28x28
 * - 已添加模型列表：cornerRadius:10, fill:#FAFAF8, height:52
 *   - 模型图标：cornerRadius:8, fill:#E8F5EE, 32x32
 *   - 编辑/删除按钮：cornerRadius:6, 28x28
 * - 表单字段：label fontSize:13 fontWeight:600, input height:40 cornerRadius:8
 * - Footer：height:80
 *   - 测试连接按钮：cornerRadius:8, fill:#F5F4F1, height:36
 *   - 取消按钮：cornerRadius:8, fill:#F5F4F1, height:36
 *   - 保存按钮：cornerRadius:8, fill:#3D8A5A, height:36
 */

import { useState, useCallback, useEffect } from 'react';
import {
  Cpu, X, Sparkles, Pencil, Trash2, Zap, Check, Eye, EyeOff, Loader2,
} from 'lucide-react';
import { cn } from '../ui/utils';
import type { CustomModelItem } from '../../utils/tauri';

export interface CustomModelModalProps {
  open: boolean;
  onClose: () => void;
  customModels: CustomModelItem[];
  onSave: (params: ModelFormData) => Promise<void>;
  onUpdate: (params: ModelFormData & { original_model_id: string }) => Promise<void>;
  onDelete: (modelId: string) => Promise<void>;
  onTestConnection: (params: {
    api_base_url: string;
    api_key: string;
    model_id: string;
  }) => Promise<{ success: boolean; message: string }>;
}

export interface ModelFormData {
  display_name: string;
  api_base_url: string;
  api_key: string;
  model_id: string;
}

const EMPTY_FORM: ModelFormData = {
  display_name: '',
  api_base_url: '',
  api_key: '',
  model_id: '',
};

export function CustomModelModal({
  open,
  onClose,
  customModels,
  onSave,
  onUpdate,
  onDelete,
  onTestConnection,
}: CustomModelModalProps) {
  const [form, setForm] = useState<ModelFormData>(EMPTY_FORM);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [showApiKey, setShowApiKey] = useState(false);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ success: boolean; message: string } | null>(null);
  const [error, setError] = useState<string | null>(null);

  // 重置表单
  const resetForm = useCallback(() => {
    setForm(EMPTY_FORM);
    setEditingId(null);
    setShowApiKey(false);
    setTestResult(null);
    setError(null);
  }, []);

  // 关闭时重置
  useEffect(() => {
    if (!open) resetForm();
  }, [open, resetForm]);

  const handleEdit = (model: CustomModelItem) => {
    setForm({
      display_name: model.display_name,
      api_base_url: model.api_base_url,
      api_key: model.api_key,
      model_id: model.model_id,
    });
    setEditingId(model.model_id);
    setTestResult(null);
    setError(null);
  };

  const handleDelete = async (modelId: string) => {
    try {
      await onDelete(modelId);
      if (editingId === modelId) resetForm();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleTestConnection = async () => {
    if (!form.api_base_url || !form.api_key || !form.model_id) {
      setTestResult({ success: false, message: '请填写 API Endpoint、API Key 和模型 ID' });
      return;
    }
    setTesting(true);
    setTestResult(null);
    try {
      const result = await onTestConnection({
        api_base_url: form.api_base_url,
        api_key: form.api_key,
        model_id: form.model_id,
      });
      setTestResult(result);
    } catch (err) {
      setTestResult({ success: false, message: err instanceof Error ? err.message : String(err) });
    } finally {
      setTesting(false);
    }
  };

  const handleSave = async () => {
    if (!form.display_name.trim() || !form.api_base_url.trim() || !form.model_id.trim()) {
      setError('请填写模型名称、API Endpoint 和模型 ID');
      return;
    }
    setSaving(true);
    setError(null);
    try {
      if (editingId) {
        await onUpdate({ ...form, original_model_id: editingId });
      } else {
        await onSave(form);
      }
      resetForm();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const updateField = (field: keyof ModelFormData, value: string) => {
    setForm((prev) => ({ ...prev, [field]: value }));
  };

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* 遮罩 */}
      <div className="absolute inset-0 bg-black/40" onClick={onClose} />

      {/* 弹窗 */}
      <div
        className={cn(
          'relative z-10 flex w-[600px] flex-col overflow-hidden',
          'rounded-2xl bg-white',
          'shadow-[0_24px_80px_rgba(0,0,0,0.12),0_8px_20px_rgba(0,0,0,0.04)]',
        )}
        style={{ maxHeight: 'min(652px, 90vh)' }}
        role="dialog"
        aria-modal="true"
        aria-label="自定义模型"
      >
        {/* ── Header ── */}
        <div className="flex h-[60px] shrink-0 items-center justify-between border-b border-border px-6">
          <div className="flex items-center gap-2.5">
            <Cpu className="size-[18px] text-primary" />
            <span className="text-base font-bold text-foreground">
              {editingId ? '编辑模型' : '自定义模型'}
            </span>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="flex size-7 items-center justify-center rounded-lg bg-[#F5F4F2] transition-colors hover:bg-accent"
            aria-label="关闭"
          >
            <X className="size-4 text-[#6D6C6A]" />
          </button>
        </div>

        {/* ── Body (scrollable) ── */}
        <div className="flex-1 overflow-y-auto">
          {/* 已添加的模型列表 */}
          {customModels.length > 0 && (
            <div className="px-6 pb-3 pt-4">
              <span className="text-[11px] font-semibold tracking-[0.5px] text-muted-foreground">
                已添加的模型
              </span>
              <div className="mt-2 space-y-2">
                {customModels.map((model) => (
                  <ModelListItem
                    key={model.model_id}
                    model={model}
                    isEditing={editingId === model.model_id}
                    onEdit={() => handleEdit(model)}
                    onDelete={() => handleDelete(model.model_id)}
                  />
                ))}
              </div>
            </div>
          )}

          {/* 分隔线 */}
          {customModels.length > 0 && <div className="h-px bg-[#EEEDE9]" />}

          {/* 添加新模型表单 */}
          <div className="space-y-4 px-6 pb-5 pt-4">
            <span className="text-[11px] font-semibold tracking-[0.5px] text-muted-foreground">
              {editingId ? '编辑模型信息' : '添加新模型'}
            </span>

            {/* 模型名称 */}
            <FormField label="模型名称">
              <input
                type="text"
                value={form.display_name}
                onChange={(e) => updateField('display_name', e.target.value)}
                placeholder="例如：My Claude"
                className="form-input"
              />
            </FormField>

            {/* API Endpoint */}
            <FormField label="API Endpoint">
              <input
                type="text"
                value={form.api_base_url}
                onChange={(e) => updateField('api_base_url', e.target.value)}
                placeholder="https://api.anthropic.com/v1"
                className="form-input"
              />
            </FormField>

            {/* API Key */}
            <FormField label="API Key">
              <div className="relative">
                <input
                  type={showApiKey ? 'text' : 'password'}
                  value={form.api_key}
                  onChange={(e) => updateField('api_key', e.target.value)}
                  placeholder="sk-ant-••••••••••••••••••••••"
                  className="form-input pr-10"
                />
                <button
                  type="button"
                  onClick={() => setShowApiKey(!showApiKey)}
                  className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
                  aria-label={showApiKey ? '隐藏 API Key' : '显示 API Key'}
                >
                  {showApiKey ? (
                    <EyeOff className="size-[15px]" />
                  ) : (
                    <Eye className="size-[15px]" />
                  )}
                </button>
              </div>
            </FormField>

            {/* 模型 ID */}
            <FormField
              label="模型 ID"
              hint="填写模型提供商的模型标识符"
            >
              <input
                type="text"
                value={form.model_id}
                onChange={(e) => updateField('model_id', e.target.value)}
                placeholder="例如：claude-3-5-sonnet-20241022"
                className="form-input"
                disabled={!!editingId}
              />
            </FormField>

            {/* 错误/测试结果 */}
            {error && (
              <p className="text-[12px] text-destructive">{error}</p>
            )}
            {testResult && (
              <p
                className={cn(
                  'text-[12px]',
                  testResult.success ? 'text-primary' : 'text-destructive',
                )}
              >
                {testResult.message}
              </p>
            )}
          </div>
        </div>

        {/* ── Footer ── */}
        <div className="flex h-20 shrink-0 items-center justify-between border-t border-border px-6">
          <button
            type="button"
            onClick={handleTestConnection}
            disabled={testing}
            className={cn(
              'flex h-9 items-center gap-1.5 rounded-lg border border-border',
              'bg-[#F5F4F1] px-4 text-[13px] font-medium text-[#4A4947]',
              'transition-colors hover:bg-accent disabled:opacity-50',
            )}
          >
            {testing ? (
              <Loader2 className="size-3.5 animate-spin" />
            ) : (
              <Zap className="size-3.5 text-[#6D6C6A]" />
            )}
            测试连接
          </button>

          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={editingId ? resetForm : onClose}
              className={cn(
                'flex h-9 items-center rounded-lg bg-[#F5F4F1] px-4',
                'text-[13px] font-medium text-[#6D6C6A]',
                'transition-colors hover:bg-accent',
              )}
            >
              取消
            </button>
            <button
              type="button"
              onClick={handleSave}
              disabled={saving}
              className={cn(
                'flex h-9 items-center gap-1.5 rounded-lg bg-primary px-4',
                'text-[13px] font-semibold text-white',
                'transition-colors hover:bg-primary/90 disabled:opacity-50',
              )}
            >
              {saving ? (
                <Loader2 className="size-3.5 animate-spin" />
              ) : (
                <Check className="size-3.5" />
              )}
              保存模型
            </button>
          </div>
        </div>
      </div>

      {/* 表单输入框样式 */}
      <style>{`
        .form-input {
          width: 100%;
          height: 40px;
          padding: 0 14px;
          border-radius: 8px;
          border: 1px solid var(--border);
          background: white;
          font-size: 13px;
          color: var(--foreground);
          outline: none;
          transition: border-color 0.15s;
        }
        .form-input::placeholder {
          color: #B0AFAC;
        }
        .form-input:focus {
          border-color: #3D8A5A;
          border-width: 1.5px;
        }
        .form-input:disabled {
          opacity: 0.5;
          cursor: not-allowed;
        }
      `}</style>
    </div>
  );
}

/* ── 子组件 ── */

function FormField({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="space-y-1.5">
      <label className="text-[13px] font-semibold text-[#4A4947]">{label}</label>
      {hint && (
        <p className="text-[11px] text-[#B0AFAC]">{hint}</p>
      )}
      {children}
    </div>
  );
}

function ModelListItem({
  model,
  isEditing,
  onEdit,
  onDelete,
}: {
  model: CustomModelItem;
  isEditing: boolean;
  onEdit: () => void;
  onDelete: () => void;
}) {
  return (
    <div
      className={cn(
        'flex h-[52px] items-center justify-between rounded-[10px] px-4',
        'border bg-[#FAFAF8]',
        isEditing ? 'border-primary' : 'border-border',
      )}
    >
      <div className="flex items-center gap-2.5">
        <div className="flex size-8 items-center justify-center rounded-lg bg-[#E8F5EE]">
          <Sparkles className="size-3.5 text-primary" />
        </div>
        <div className="flex flex-col gap-0.5">
          <span className="text-[13px] font-semibold text-foreground">
            {model.display_name}
          </span>
          <span className="text-[11px] text-muted-foreground">
            {model.api_base_url}
          </span>
        </div>
      </div>

      <div className="flex items-center gap-1.5">
        {isEditing && (
          <span className="rounded-full bg-[#E8F5EE] px-2 py-0.5 text-[11px] font-semibold text-primary">
            编辑中
          </span>
        )}
        <button
          type="button"
          onClick={onEdit}
          className="flex size-7 items-center justify-center rounded-md bg-[#F5F4F1] transition-colors hover:bg-accent"
          aria-label="编辑模型"
        >
          <Pencil className="size-[13px] text-[#6D6C6A]" />
        </button>
        <button
          type="button"
          onClick={onDelete}
          className="flex size-7 items-center justify-center rounded-md bg-[#FFF0F0] transition-colors hover:bg-red-100"
          aria-label="删除模型"
        >
          <Trash2 className="size-[13px] text-[#E53E3E]" />
        </button>
      </div>
    </div>
  );
}
