import { describe, it, expect } from 'vitest';
import {
  getChangeTypeText,
  getChangeTypeColor,
  getRuleTypeText,
  getRuleTypeColor,
  getFieldLabel,
  formatChangeValue,
  CHANGE_TYPE_OPTIONS,
  RULE_TYPE_OPTIONS,
} from './policyChanges';

describe('policyChanges 常量', () => {
  describe('CHANGE_TYPE_OPTIONS', () => {
    it('应该包含所有变更类型', () => {
      const types = CHANGE_TYPE_OPTIONS.map((o) => o.value);
      expect(types).toContain('create');
      expect(types).toContain('update');
      expect(types).toContain('delete');
      expect(types).toContain('enable');
      expect(types).toContain('disable');
      expect(types).toContain('import');
    });

    it('每个选项应该有 label 和 color', () => {
      CHANGE_TYPE_OPTIONS.forEach((opt) => {
        expect(opt.label).toBeTruthy();
        expect(opt.color).toBeTruthy();
      });
    });
  });

  describe('RULE_TYPE_OPTIONS', () => {
    it('应该包含所有规则类型', () => {
      const types = RULE_TYPE_OPTIONS.map((o) => o.value);
      expect(types).toContain('dlp_rule');
      expect(types).toContain('sensitive_op');
      expect(types).toContain('dictionary');
    });
  });

  describe('getChangeTypeText', () => {
    it('应该返回正确的中文文本', () => {
      expect(getChangeTypeText('create')).toBe('创建');
      expect(getChangeTypeText('update')).toBe('修改');
      expect(getChangeTypeText('delete')).toBe('删除');
      expect(getChangeTypeText('enable')).toBe('启用');
      expect(getChangeTypeText('disable')).toBe('禁用');
      expect(getChangeTypeText('import')).toBe('导入');
    });

    it('未知类型应该返回原始值', () => {
      expect(getChangeTypeText('unknown')).toBe('unknown');
    });
  });

  describe('getChangeTypeColor', () => {
    it('应该返回正确的颜色', () => {
      expect(getChangeTypeColor('create')).toBe('success');
      expect(getChangeTypeColor('delete')).toBe('error');
    });

    it('未知类型应该返回 default', () => {
      expect(getChangeTypeColor('unknown')).toBe('default');
    });
  });

  describe('getRuleTypeText', () => {
    it('应该返回正确的中文文本', () => {
      expect(getRuleTypeText('dlp_rule')).toBe('DLP 规则');
      expect(getRuleTypeText('sensitive_op')).toBe('敏感操作');
      expect(getRuleTypeText('dictionary')).toBe('字典');
    });

    it('未知类型应该返回原始值', () => {
      expect(getRuleTypeText('unknown')).toBe('unknown');
    });
  });

  describe('getRuleTypeColor', () => {
    it('应该返回正确的颜色', () => {
      expect(getRuleTypeColor('dlp_rule')).toBe('blue');
      expect(getRuleTypeColor('sensitive_op')).toBe('orange');
      expect(getRuleTypeColor('dictionary')).toBe('green');
    });
  });

  describe('getFieldLabel', () => {
    it('应该返回字段的中文名', () => {
      expect(getFieldLabel('name')).toBe('名称');
      expect(getFieldLabel('pattern')).toBe('匹配模式');
      expect(getFieldLabel('severity')).toBe('严重级别');
      expect(getFieldLabel('enabled')).toBe('状态');
      expect(getFieldLabel('risk_level')).toBe('风险等级');
    });

    it('未知字段应该返回原始值', () => {
      expect(getFieldLabel('unknown_field')).toBe('unknown_field');
    });
  });

  describe('formatChangeValue', () => {
    it('null/undefined 应该返回 -', () => {
      expect(formatChangeValue(null)).toBe('-');
      expect(formatChangeValue(undefined)).toBe('-');
    });

    it('布尔值应该返回中文', () => {
      expect(formatChangeValue(true)).toBe('是');
      expect(formatChangeValue(false)).toBe('否');
    });

    it('数组应该用逗号连接', () => {
      expect(formatChangeValue(['a', 'b', 'c'])).toBe('a, b, c');
    });

    it('对象应该 JSON 格式化', () => {
      const result = formatChangeValue({ key: 'value' });
      expect(result).toContain('"key"');
      expect(result).toContain('"value"');
    });

    it('字符串应该直接返回', () => {
      expect(formatChangeValue('hello')).toBe('hello');
    });

    it('数字应该转为字符串', () => {
      expect(formatChangeValue(42)).toBe('42');
    });
  });
});
