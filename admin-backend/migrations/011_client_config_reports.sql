-- 客户端配置表
-- 管理端可为不同客户端/用户组下发不同的配置
CREATE TABLE IF NOT EXISTS client_configs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- NULL 表示全局默认配置，非 NULL 表示特定客户端配置
    client_id UUID REFERENCES registered_clients(id) ON DELETE CASCADE,
    -- LLM 配置
    llm_backend VARCHAR(50),
    llm_api_key TEXT,
    llm_model VARCHAR(100),
    llm_base_url TEXT,
    -- 安全策略
    safety_enabled BOOLEAN,
    -- 功能开关
    skills_enabled BOOLEAN,
    extensions_enabled BOOLEAN,
    -- 限制
    max_cost_per_day_cents BIGINT,
    -- 版本管理
    config_version BIGINT NOT NULL DEFAULT 1,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- 全局默认配置唯一约束（client_id 为 NULL 时只能有一条）
CREATE UNIQUE INDEX IF NOT EXISTS idx_client_configs_global
    ON client_configs (client_id) WHERE client_id IS NULL;

CREATE INDEX IF NOT EXISTS idx_client_configs_client_id
    ON client_configs (client_id);

-- 客户端上报事件表
CREATE TABLE IF NOT EXISTS client_reports (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    client_id UUID REFERENCES registered_clients(id) ON DELETE SET NULL,
    report_type VARCHAR(50) NOT NULL,
    payload JSONB NOT NULL,
    received_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_client_reports_client_id ON client_reports(client_id);
CREATE INDEX IF NOT EXISTS idx_client_reports_type ON client_reports(report_type);
CREATE INDEX IF NOT EXISTS idx_client_reports_received_at ON client_reports(received_at DESC);
