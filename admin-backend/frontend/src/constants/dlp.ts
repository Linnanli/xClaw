/**
 * DLP 规则相关常量定义
 */

/**
 * 规则类型选项
 */
export const DLP_RULE_TYPE_OPTIONS = [
  { value: 'regex', label: '正则表达式', description: '使用正则表达式匹配文本模式' },
  { value: 'keyword', label: '关键字匹配', description: '使用关键字列表匹配文本内容' },
  { value: 'dictionary', label: '字典匹配', description: '使用预定义字典中的关键字匹配' },
] as const;

/**
 * 关键字匹配模式选项
 */
export const KEYWORD_MATCH_MODE_OPTIONS = [
  { value: 'exact', label: '精确匹配', description: '完全匹配关键字' },
  { value: 'contains', label: '包含匹配', description: '文本中包含关键字即匹配' },
  { value: 'whole_word', label: '全词匹配', description: '匹配完整的词（前后有边界）' },
] as const;

/**
 * 严重级别选项
 */
export const DLP_SEVERITY_OPTIONS = [
  { value: 'low', label: '低', color: 'blue' },
  { value: 'medium', label: '中', color: 'orange' },
  { value: 'high', label: '高', color: 'red' },
  { value: 'critical', label: '严重', color: 'purple' },
] as const;

/**
 * 分类选项
 */
export const DLP_CATEGORY_OPTIONS = [
  { value: 'pii', label: '个人身份信息 (PII)', description: '身份证、护照、驾照等' },
  { value: 'financial', label: '金融信息', description: '银行卡号、信用卡号等' },
  { value: 'health', label: '健康医疗信息', description: '病历、诊断记录等' },
  { value: 'credential', label: '凭证信息', description: '密码、API密钥、Token等' },
  { value: 'confidential', label: '机密信息', description: '商业机密、内部文档等' },
  { value: 'other', label: '其他', description: '其他敏感信息' },
] as const;

/**
 * 严重级别映射
 */
export const SEVERITY_MAP: Record<string, { label: string; color: string }> = {
  low: { label: '低', color: 'blue' },
  medium: { label: '中', color: 'orange' },
  high: { label: '高', color: 'red' },
  critical: { label: '严重', color: 'purple' },
};

/**
 * 分类映射
 */
export const CATEGORY_MAP: Record<string, string> = {
  pii: '个人身份信息',
  financial: '金融信息',
  health: '健康医疗信息',
  credential: '凭证信息',
  confidential: '机密信息',
  other: '其他',
};

/**
 * 获取严重级别文本
 */
export const getSeverityText = (severity: string): string => {
  return SEVERITY_MAP[severity]?.label || severity;
};

/**
 * 获取严重级别颜色
 */
export const getSeverityColor = (severity: string): string => {
  return SEVERITY_MAP[severity]?.color || 'default';
};

/**
 * 获取分类文本
 */
export const getCategoryText = (category: string): string => {
  return CATEGORY_MAP[category] || category;
};
