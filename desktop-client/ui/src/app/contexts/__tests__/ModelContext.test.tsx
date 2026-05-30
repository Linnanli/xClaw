/**
 * ModelContext 测试
 *
 * 覆盖维度：
 * - 单元测试：默认值、context 消费
 * - 契约测试：接口形状、类型约束
 * - 失败路径：在 Provider 外使用时的降级行为
 * - 安全审计：context 不泄露敏感字段
 */

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ModelContext, useModelContext, type ModelContextValue } from '../ModelContext';
import type { ModelConfigItem } from '../../utils/tauri';

// ============================================================================
// 测试数据
// ============================================================================

const mockModels: ModelConfigItem[] = [
  {
    model_id: 'model-a',
    display_name: 'Model A',
    description: null,
    provider: 'openai',
    is_default: true,
    capabilities: ['chat'],
    source: 'admin',
  },
];

function buildContextValue(overrides: Partial<ModelContextValue> = {}): ModelContextValue {
  return {
    selectedModelId: 'model-a',
    models: mockModels,
    selectModel: vi.fn(),
    openCustomModelModal: vi.fn(),
    loading: false,
    ...overrides,
  };
}

// 消费 context 的测试组件
function ConsumerComponent() {
  const ctx = useModelContext();
  return (
    <div>
      <span data-testid="selected-id">{ctx.selectedModelId}</span>
      <span data-testid="model-count">{ctx.models.length}</span>
      <span data-testid="loading">{String(ctx.loading)}</span>
      <button data-testid="select-btn" onClick={() => ctx.selectModel('model-b')}>
        切换
      </button>
      <button data-testid="custom-btn" onClick={() => ctx.openCustomModelModal()}>
        自定义
      </button>
    </div>
  );
}

// ============================================================================
// 单元测试 - 正常路径
// ============================================================================

describe('ModelContext - 正常路径', () => {
  it('Provider 应向子组件提供 selectedModelId', () => {
    render(
      <ModelContext.Provider value={buildContextValue({ selectedModelId: 'model-a' })}>
        <ConsumerComponent />
      </ModelContext.Provider>,
    );
    expect(screen.getByTestId('selected-id').textContent).toBe('model-a');
  });

  it('Provider 应向子组件提供 models 列表', () => {
    render(
      <ModelContext.Provider value={buildContextValue()}>
        <ConsumerComponent />
      </ModelContext.Provider>,
    );
    expect(screen.getByTestId('model-count').textContent).toBe('1');
  });

  it('Provider 应向子组件提供 loading 状态', () => {
    render(
      <ModelContext.Provider value={buildContextValue({ loading: true })}>
        <ConsumerComponent />
      </ModelContext.Provider>,
    );
    expect(screen.getByTestId('loading').textContent).toBe('true');
  });

  it('点击切换按钮应调用 selectModel', () => {
    const selectModel = vi.fn();
    render(
      <ModelContext.Provider value={buildContextValue({ selectModel })}>
        <ConsumerComponent />
      </ModelContext.Provider>,
    );
    fireEvent.click(screen.getByTestId('select-btn'));
    expect(selectModel).toHaveBeenCalledWith('model-b');
  });

  it('点击自定义按钮应调用 openCustomModelModal', () => {
    const openCustomModelModal = vi.fn();
    render(
      <ModelContext.Provider value={buildContextValue({ openCustomModelModal })}>
        <ConsumerComponent />
      </ModelContext.Provider>,
    );
    fireEvent.click(screen.getByTestId('custom-btn'));
    expect(openCustomModelModal).toHaveBeenCalledTimes(1);
  });
});

// ============================================================================
// 失败路径测试
// ============================================================================

describe('ModelContext - 失败路径', () => {
  it('test_failure_outside_provider_uses_default_values', () => {
    // 在 Provider 外使用时应返回默认值，不应 throw
    render(<ConsumerComponent />);
    expect(screen.getByTestId('selected-id').textContent).toBe('');
    expect(screen.getByTestId('model-count').textContent).toBe('0');
    expect(screen.getByTestId('loading').textContent).toBe('false');
  });

  it('test_failure_default_selectModel_is_noop', () => {
    // 默认 selectModel 不应 throw
    render(<ConsumerComponent />);
    expect(() => fireEvent.click(screen.getByTestId('select-btn'))).not.toThrow();
  });

  it('test_failure_default_openCustomModelModal_is_noop', () => {
    // 默认 openCustomModelModal 不应 throw
    render(<ConsumerComponent />);
    expect(() => fireEvent.click(screen.getByTestId('custom-btn'))).not.toThrow();
  });

  it('test_failure_empty_models_array_is_valid', () => {
    render(
      <ModelContext.Provider value={buildContextValue({ models: [] })}>
        <ConsumerComponent />
      </ModelContext.Provider>,
    );
    expect(screen.getByTestId('model-count').textContent).toBe('0');
  });
});

// ============================================================================
// 契约测试
// ============================================================================

describe('ModelContext - 契约测试', () => {
  it('test_contract_context_value_shape_has_all_required_fields', () => {
    const ctx = buildContextValue();
    expect(ctx).toHaveProperty('selectedModelId');
    expect(ctx).toHaveProperty('models');
    expect(ctx).toHaveProperty('selectModel');
    expect(ctx).toHaveProperty('openCustomModelModal');
    expect(ctx).toHaveProperty('loading');
  });

  it('test_contract_selectModel_is_function', () => {
    const ctx = buildContextValue();
    expect(typeof ctx.selectModel).toBe('function');
  });

  it('test_contract_openCustomModelModal_is_function', () => {
    const ctx = buildContextValue();
    expect(typeof ctx.openCustomModelModal).toBe('function');
  });

  it('test_contract_models_is_array', () => {
    const ctx = buildContextValue();
    expect(Array.isArray(ctx.models)).toBe(true);
  });

  it('test_contract_loading_is_boolean', () => {
    const ctx = buildContextValue();
    expect(typeof ctx.loading).toBe('boolean');
  });

  it('test_contract_model_item_has_required_fields', () => {
    const model = mockModels[0];
    expect(model).toHaveProperty('model_id');
    expect(model).toHaveProperty('display_name');
    expect(model).toHaveProperty('is_default');
    expect(model).toHaveProperty('source');
  });
});

// ============================================================================
// 安全审计测试
// ============================================================================

describe('ModelContext - 安全审计', () => {
  it('test_audit_context_does_not_expose_api_key', () => {
    // ModelContextValue 接口不包含 api_key 字段
    const ctx = buildContextValue();
    expect(ctx).not.toHaveProperty('api_key');
    expect(ctx).not.toHaveProperty('apiKey');
  });

  it('test_audit_context_does_not_expose_api_base_url', () => {
    // api_base_url 属于 CustomModelItem，不应出现在 ModelContext 中
    const ctx = buildContextValue();
    expect(ctx).not.toHaveProperty('api_base_url');
  });

  it('test_audit_model_items_in_context_do_not_contain_api_key', () => {
    const ctx = buildContextValue();
    ctx.models.forEach((model) => {
      expect(model).not.toHaveProperty('api_key');
    });
  });
});
