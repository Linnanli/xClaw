-- 费用配额管理 + 模型单价
--
-- 变更内容：
-- 1. model_configs 表增加输入/输出单价字段
-- 2. 新建 usage_records 费用消耗记录表
-- 3. 新建 quota_configs 配额配置表（组织/部门/个人层级）

-- 1. 模型单价（单位：分/千Token）
ALTER TABLE model_configs
    ADD COLUMN IF NOT EXISTS input_price_per_1k_cents INTEGER,
    ADD COLUMN IF NOT EXISTS output_price_per_1k_cents INTEGER;

-- 2. 费用消耗记录（每次 AI 请求完成后写入一条）
CREATE TABLE IF NOT EXISTS usage_records (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    department_id UUID REFERENCES departments(id) ON DELETE SET NULL,
    model_config_id UUID REFERENCES model_configs(id) ON DELETE SET NULL,
    model_id VARCHAR(100) NOT NULL,
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    cost_cents INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_usage_records_user_date
    ON usage_records(user_id, created_at);
CREATE INDEX IF NOT EXISTS idx_usage_records_dept_date
    ON usage_records(department_id, created_at);
CREATE INDEX IF NOT EXISTS idx_usage_records_model
    ON usage_records(model_config_id);
CREATE INDEX IF NOT EXISTS idx_usage_records_created
    ON usage_records(created_at);

-- 3. 配额配置（支持组织/部门/个人三个层级）
--    scope: 'org' | 'department' | 'user'
--    scope_id: 对应的 UUID（org 级别为 NULL）
CREATE TABLE IF NOT EXISTS quota_configs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    scope VARCHAR(20) NOT NULL CHECK (scope IN ('org', 'department', 'user')),
    scope_id UUID,
    daily_limit_cents INTEGER,
    monthly_budget_cents INTEGER,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE (scope, scope_id)
);
