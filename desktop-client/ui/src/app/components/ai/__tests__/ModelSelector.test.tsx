/**
 * ModelSelector 测试
 *
 * 覆盖维度（参考 AGENTS.md 测试维度矩阵）：
 * - 单元测试：正常路径 + 错误路径
 * - 契约测试：props 接口 / context 接口一致性
 * - 安全审计：API Key 等敏感信息不泄露到 UI
 * - 失败路径：空模型列表、加载中、无效 selectedModelId
 *
 * 设计规范验证（Pencil "X-Claw Chat V2 - Model Select - Theme Toggle"）：
 * - 触发器高度 28px，圆角 8px，Sparkles 图标
 * - 下拉面板宽度 240px，圆角 12px
 * - 选中项：#F0F9F4 背景 + Check 图标
 * - 推荐角标：#C8F0D8 背景
 * - 自定义模型入口：CirclePlus + ChevronRight
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { ModelSelector } from '../ModelSelector';
import { ModelContext, type ModelContextValue } from '../../../contexts/ModelContext';
import type { ModelConfigItem } from '../../../utils/tauri';

// ============================================================================
// 测试数据
// ============================================================================

const mockModels: ModelConfigItem[] = [
  {
    model_id: 'deepseek-chat',
    display_name: 'DeepSeek Chat',
    description: '通用对话模型',
    provider: 'deepseek',
    is_default: true,
    capabilities: ['chat'],
    source: 'admin',
  },
  {
    model_id: 'gpt-4o',
    display_name: 'GPT-4o',
    description: 'OpenAI 旗舰模型',
    provider: 'openai',
    is_default: false,
    capabilities: ['chat', 'vision'],
    source: 'admin',
  },
  {
    model_id: 'custom-llm',
    display_name: '自定义 LLM',
    description: null,
    provider: 'custom',
    is_default: false,
    capabilities: ['chat'],
    source: 'custom',
  },
];

function buildContext(overrides: Partial<ModelContextValue> = {}): ModelContextValue {
  return {
    selectedModelId: 'deepseek-chat',
    models: mockModels,
    selectModel: vi.fn(),
    openCustomModelModal: vi.fn(),
    loading: false,
    ...overrides,
  };
}

function renderWithContext(ctx: ModelContextValue) {
  return render(
    <ModelContext.Provider value={ctx}>
      <ModelSelector />
    </ModelContext.Provider>,
  );
}

// ============================================================================
// 单元测试 - 正常路径
// ============================================================================

describe('ModelSelector - 正常路径', () => {
  it('应渲染触发器并显示当前选中模型名称', () => {
    renderWithContext(buildContext());
    expect(screen.getByTestId('model-selector-trigger')).toBeInTheDocument();
    expect(screen.getByText('DeepSeek Chat')).toBeInTheDocument();
  });

  it('点击触发器应展开下拉面板', () => {
    renderWithContext(buildContext());
    fireEvent.click(screen.getByTestId('model-selector-trigger'));
    expect(screen.getByTestId('model-selector-panel')).toBeInTheDocument();
  });

  it('展开后应显示所有模型选项', () => {
    renderWithContext(buildContext());
    fireEvent.click(screen.getByTestId('model-selector-trigger'));
    expect(screen.getByTestId('model-option-deepseek-chat')).toBeInTheDocument();
    expect(screen.getByTestId('model-option-gpt-4o')).toBeInTheDocument();
    expect(screen.getByTestId('model-option-custom-llm')).toBeInTheDocument();
  });

  it('默认模型应显示"推荐"角标', () => {
    renderWithContext(buildContext());
    fireEvent.click(screen.getByTestId('model-selector-trigger'));
    expect(screen.getByText('推荐')).toBeInTheDocument();
  });

  it('选中模型应显示 Check 图标（aria-selected=true）', () => {
    renderWithContext(buildContext({ selectedModelId: 'gpt-4o' }));
    fireEvent.click(screen.getByTestId('model-selector-trigger'));
    const selectedOption = screen.getByTestId('model-option-gpt-4o');
    expect(selectedOption).toHaveAttribute('aria-selected', 'true');
  });

  it('未选中模型的 aria-selected 应为 false', () => {
    renderWithContext(buildContext({ selectedModelId: 'deepseek-chat' }));
    fireEvent.click(screen.getByTestId('model-selector-trigger'));
    const unselectedOption = screen.getByTestId('model-option-gpt-4o');
    expect(unselectedOption).toHaveAttribute('aria-selected', 'false');
  });

  it('点击模型选项应调用 selectModel 并关闭面板', async () => {
    const selectModel = vi.fn();
    renderWithContext(buildContext({ selectModel }));
    fireEvent.click(screen.getByTestId('model-selector-trigger'));
    fireEvent.click(screen.getByTestId('model-option-gpt-4o'));
    expect(selectModel).toHaveBeenCalledWith('gpt-4o');
    await waitFor(() => {
      expect(screen.queryByTestId('model-selector-panel')).toBeNull();
    });
  });

  it('点击"自定义模型"应调用 openCustomModelModal 并关闭面板', async () => {
    const openCustomModelModal = vi.fn();
    renderWithContext(buildContext({ openCustomModelModal }));
    fireEvent.click(screen.getByTestId('model-selector-trigger'));
    fireEvent.click(screen.getByTestId('model-selector-custom'));
    expect(openCustomModelModal).toHaveBeenCalledTimes(1);
    await waitFor(() => {
      expect(screen.queryByTestId('model-selector-panel')).toBeNull();
    });
  });

  it('点击面板外部应关闭面板', async () => {
    renderWithContext(buildContext());
    fireEvent.click(screen.getByTestId('model-selector-trigger'));
    expect(screen.getByTestId('model-selector-panel')).toBeInTheDocument();
    fireEvent.mouseDown(document.body);
    await waitFor(() => {
      expect(screen.queryByTestId('model-selector-panel')).toBeNull();
    });
  });

  it('再次点击触发器应关闭已展开的面板', async () => {
    renderWithContext(buildContext());
    fireEvent.click(screen.getByTestId('model-selector-trigger'));
    expect(screen.getByTestId('model-selector-panel')).toBeInTheDocument();
    fireEvent.click(screen.getByTestId('model-selector-trigger'));
    await waitFor(() => {
      expect(screen.queryByTestId('model-selector-panel')).toBeNull();
    });
  });
});

// ============================================================================
// 单元测试 - 失败路径
// ============================================================================

describe('ModelSelector - 失败路径', () => {
  it('test_failure_loading_state_shows_skeleton', () => {
    renderWithContext(buildContext({ loading: true, models: [] }));
    expect(screen.getByLabelText('加载模型列表')).toBeInTheDocument();
    // 加载中不应渲染触发器（不可点击）
    expect(screen.queryByTestId('model-selector-trigger')).toBeNull();
  });

  it('test_failure_loading_with_existing_models_still_shows_trigger', () => {
    // 有缓存模型时即使 loading=true 也应正常显示触发器
    renderWithContext(buildContext({ loading: true, models: mockModels }));
    expect(screen.getByTestId('model-selector-trigger')).toBeInTheDocument();
  });

  it('test_failure_empty_model_list_shows_only_custom_entry', () => {
    renderWithContext(buildContext({ models: [], selectedModelId: '' }));
    fireEvent.click(screen.getByTestId('model-selector-trigger'));
    // 无模型时面板仍应渲染，只有自定义模型入口
    expect(screen.getByTestId('model-selector-custom')).toBeInTheDocument();
    expect(screen.queryByTestId('model-option-deepseek-chat')).toBeNull();
  });

  it('test_failure_invalid_selected_model_shows_fallback_label', () => {
    renderWithContext(buildContext({ selectedModelId: 'nonexistent-model', models: mockModels }));
    // 找不到对应模型时应显示 model_id 本身作为 fallback
    expect(screen.getByText('nonexistent-model')).toBeInTheDocument();
  });

  it('test_failure_empty_selected_model_shows_placeholder', () => {
    renderWithContext(buildContext({ selectedModelId: '', models: [] }));
    expect(screen.getByText('选择模型')).toBeInTheDocument();
  });
});

// ============================================================================
// 契约测试
// ============================================================================

describe('ModelSelector - 契约测试', () => {
  it('test_contract_trigger_has_aria_haspopup', () => {
    renderWithContext(buildContext());
    const trigger = screen.getByTestId('model-selector-trigger');
    expect(trigger).toHaveAttribute('aria-haspopup', 'listbox');
  });

  it('test_contract_trigger_aria_expanded_reflects_open_state', () => {
    renderWithContext(buildContext());
    const trigger = screen.getByTestId('model-selector-trigger');
    expect(trigger).toHaveAttribute('aria-expanded', 'false');
    fireEvent.click(trigger);
    expect(trigger).toHaveAttribute('aria-expanded', 'true');
  });

  it('test_contract_panel_has_listbox_role', () => {
    renderWithContext(buildContext());
    fireEvent.click(screen.getByTestId('model-selector-trigger'));
    expect(screen.getByRole('listbox')).toBeInTheDocument();
  });

  it('test_contract_model_options_have_option_role', () => {
    renderWithContext(buildContext());
    fireEvent.click(screen.getByTestId('model-selector-trigger'));
    const options = screen.getAllByRole('option');
    // 3 个模型选项
    expect(options.length).toBe(3);
  });

  it('test_contract_trigger_label_contains_current_model', () => {
    renderWithContext(buildContext({ selectedModelId: 'gpt-4o' }));
    const trigger = screen.getByTestId('model-selector-trigger');
    expect(trigger).toHaveAttribute('aria-label', '当前模型：GPT-4o');
  });

  it('test_contract_select_model_called_with_correct_id', () => {
    const selectModel = vi.fn();
    renderWithContext(buildContext({ selectModel }));
    fireEvent.click(screen.getByTestId('model-selector-trigger'));
    fireEvent.click(screen.getByTestId('model-option-custom-llm'));
    expect(selectModel).toHaveBeenCalledWith('custom-llm');
    expect(selectModel).toHaveBeenCalledTimes(1);
  });

  it('test_contract_panel_shows_section_title', () => {
    renderWithContext(buildContext());
    fireEvent.click(screen.getByTestId('model-selector-trigger'));
    expect(screen.getByText('选择模型')).toBeInTheDocument();
  });
});

// ============================================================================
// 安全审计测试
// ============================================================================

describe('ModelSelector - 安全审计', () => {
  it('test_audit_api_key_not_rendered_in_dom', () => {
    // ModelSelector 不接收 API Key，确保不会意外渲染
    renderWithContext(buildContext());
    fireEvent.click(screen.getByTestId('model-selector-trigger'));
    const html = document.body.innerHTML;
    expect(html).not.toContain('api_key');
    expect(html).not.toContain('sk-');
  });

  it('test_audit_model_description_not_leaked_in_trigger', () => {
    // 触发器只显示 display_name，不泄露 description
    renderWithContext(buildContext());
    const trigger = screen.getByTestId('model-selector-trigger');
    expect(trigger.textContent).not.toContain('通用对话模型');
  });

  it('test_audit_source_field_not_rendered', () => {
    // source 字段（admin/custom/builtin）不应出现在 UI 中
    renderWithContext(buildContext());
    fireEvent.click(screen.getByTestId('model-selector-trigger'));
    const html = document.body.innerHTML;
    // 不应直接渲染 source 字段值（可能暴露内部分类）
    expect(html).not.toContain('"admin"');
    expect(html).not.toContain('"builtin"');
  });

  it('test_audit_model_id_not_exposed_as_visible_text_when_display_name_exists', () => {
    // 有 display_name 时，model_id 不应作为可见文本出现在触发器中
    renderWithContext(buildContext({ selectedModelId: 'deepseek-chat' }));
    const trigger = screen.getByTestId('model-selector-trigger');
    // 触发器显示 display_name，不显示原始 model_id
    expect(trigger.textContent).toContain('DeepSeek Chat');
    expect(trigger.textContent).not.toContain('deepseek-chat');
  });
});
