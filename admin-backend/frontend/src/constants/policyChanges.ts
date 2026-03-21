import type { PolicyChangeRuleType, PolicyChangeType } from '../types';

// 变更类型选项
export const CHANGE_TYPE_OPTIONS: Array<{ value: PolicyChangeType; label: string; color: string }> = [
  { value: 'create', label: '创建', color: 'success' },
  { value: 'update', label: '修改', color: 'processing' },
  { value: 'delete', label: '删除', color: 'error' },
  { value: 'enable', label: '启用', color: 'cyan' },
  { value: 'disable', label: '禁用', color: 'warning' },
  { value: 'import', label: '导入', color: 'purple' },
];

// 规则类型选项
export const RULE_TYPE_OPTIONS: Array<{ value: PolicyChangeRuleType; label: string; color: string }> = [
  { value: 'dlp_rule', label: 'DLP 规则', color: 'blue' },
  { value: 'sensitive_op', label: '敏感操作', color: 'orange' },
  { value: 'dictionary', label: '字典', color: 'green' },
];

/** 获取变更类型文本 */
export function getChangeTypeText(type: string): string {
  return CHANGE_TYPE_OPTIONS.find((o) => o.value === type)?.label || type;
}

/** 获取变更类型颜色 */
export function getChangeTypeColor(type: string): string {
  return CHANGE_TYPE_OPTIONS.find((o) => o.value === type)?.color || 'default';
}

/** 获取规则类型文本 */
export function getRuleTypeText(type: string): string {
  return RULE_TYPE_OPTIONS.find((o) => o.value === type)?.label || type;
}

/** 获取规则类型颜色 */
export function getRuleTypeColor(type: string): string {
  return RULE_TYPE_OPTIONS.find((o) => o.value === type)?.color || 'default';
}

/** 获取字段变更的中文名 */
export function getFieldLabel(field: string): string {
  const map: Record<string, string> = {
    name: '名称',
    pattern: '匹配模式',
    replacement: '替换文本',
    severity: '严重级别',
    description: '描述',
    enabled: '状态',
    category: '分类',
    rule_type: '规则类型',
    rule_config: '规则配置',
    operation_type: '操作类型',
    requires_approval: '审批要求',
    risk_level: '风险等级',
    approver_roles: '审批角色',
    keywords: '关键字',
    keyword_count: '关键字数量',
  };
  return map[field] || field;
}

/** 格式化变更值为可读文本 */
export function formatChangeValue(value: any): string {
  if (value === null || value === undefined) return '-';
  if (typeof value === 'boolean') return value ? '是' : '否';
  if (Array.isArray(value)) return value.join(', ');
  if (typeof value === 'object') return JSON.stringify(value, null, 2);
  return String(value);
}
