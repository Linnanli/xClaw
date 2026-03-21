-- 策略变更记录表
CREATE TABLE IF NOT EXISTS policy_change_records (
    id UUID PRIMARY KEY,
    rule_id UUID NOT NULL,
    rule_type VARCHAR(50) NOT NULL DEFAULT 'dlp_rule',  -- dlp_rule, sensitive_op, dictionary
    rule_name VARCHAR(255),
    change_type VARCHAR(50) NOT NULL,  -- create, update, delete, enable, disable, import
    field_changed VARCHAR(100),  -- 变更的字段名（update 时使用）
    old_value JSONB,
    new_value JSONB,
    changed_by UUID,
    changed_by_name VARCHAR(255) DEFAULT 'system',
    changed_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    reason TEXT
);

-- 索引
CREATE INDEX IF NOT EXISTS idx_policy_changes_rule_id ON policy_change_records(rule_id);
CREATE INDEX IF NOT EXISTS idx_policy_changes_rule_type ON policy_change_records(rule_type);
CREATE INDEX IF NOT EXISTS idx_policy_changes_change_type ON policy_change_records(change_type);
CREATE INDEX IF NOT EXISTS idx_policy_changes_changed_at ON policy_change_records(changed_at DESC);
CREATE INDEX IF NOT EXISTS idx_policy_changes_changed_by ON policy_change_records(changed_by);
