-- 007: 扩展敏感操作规则表字段
-- 添加 name, risk_level, description, enabled, approver_roles, updated_at 字段

ALTER TABLE sensitive_operation_rules ADD COLUMN IF NOT EXISTS name VARCHAR(100);
ALTER TABLE sensitive_operation_rules ADD COLUMN IF NOT EXISTS risk_level VARCHAR(20) DEFAULT 'medium';
ALTER TABLE sensitive_operation_rules ADD COLUMN IF NOT EXISTS description TEXT;
ALTER TABLE sensitive_operation_rules ADD COLUMN IF NOT EXISTS enabled BOOLEAN DEFAULT true;
ALTER TABLE sensitive_operation_rules ADD COLUMN IF NOT EXISTS approver_roles TEXT[] DEFAULT '{}';
ALTER TABLE sensitive_operation_rules ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ DEFAULT NOW();
ALTER TABLE sensitive_operation_rules ADD COLUMN IF NOT EXISTS created_by UUID;
ALTER TABLE sensitive_operation_rules ADD COLUMN IF NOT EXISTS updated_by UUID;

-- 为现有记录设置默认值
UPDATE sensitive_operation_rules SET name = operation_type WHERE name IS NULL;
UPDATE sensitive_operation_rules SET risk_level = 'medium' WHERE risk_level IS NULL;
UPDATE sensitive_operation_rules SET enabled = true WHERE enabled IS NULL;
UPDATE sensitive_operation_rules SET updated_at = created_at WHERE updated_at IS NULL;

-- 设置 name 为非空
ALTER TABLE sensitive_operation_rules ALTER COLUMN name SET NOT NULL;

-- 添加索引
CREATE INDEX IF NOT EXISTS idx_sensitive_ops_type ON sensitive_operation_rules(operation_type);
CREATE INDEX IF NOT EXISTS idx_sensitive_ops_enabled ON sensitive_operation_rules(enabled);
