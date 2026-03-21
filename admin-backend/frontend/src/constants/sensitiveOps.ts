/**
 * 敏感操作相关常量定义
 */

/**
 * 操作类型选项
 */
export const SENSITIVE_OP_TYPE_OPTIONS = [
  { value: 'file_operation', label: '文件操作', description: '文件删除、移动、重命名等' },
  { value: 'system_command', label: '系统命令', description: '执行系统级命令或脚本' },
  { value: 'network_access', label: '网络访问', description: '外部网络请求或数据传输' },
  { value: 'data_export', label: '数据导出', description: '导出用户数据或业务数据' },
  { value: 'config_change', label: '配置变更', description: '修改系统配置或安全策略' },
] as const;

/**
 * 风险等级选项（复用 DLP 颜色体系）
 */
export const RISK_LEVEL_OPTIONS = [
  { value: 'low', label: '低', color: 'blue' },
  { value: 'medium', label: '中', color: 'orange' },
  { value: 'high', label: '高', color: 'red' },
  { value: 'critical', label: '严重', color: 'purple' },
] as const;

/**
 * 操作类型映射
 */
export const OP_TYPE_MAP: Record<string, string> = {
  file_operation: '文件操作',
  system_command: '系统命令',
  network_access: '网络访问',
  data_export: '数据导出',
  config_change: '配置变更',
};

/**
 * 风险等级映射
 */
export const RISK_LEVEL_MAP: Record<string, { label: string; color: string }> = {
  low: { label: '低', color: 'blue' },
  medium: { label: '中', color: 'orange' },
  high: { label: '高', color: 'red' },
  critical: { label: '严重', color: 'purple' },
};

/** 获取操作类型文本 */
export const getOpTypeText = (type: string): string => OP_TYPE_MAP[type] || type;

/** 获取风险等级文本 */
export const getRiskLevelText = (level: string): string => RISK_LEVEL_MAP[level]?.label || level;

/** 获取风险等级颜色 */
export const getRiskLevelColor = (level: string): string => RISK_LEVEL_MAP[level]?.color || 'default';
