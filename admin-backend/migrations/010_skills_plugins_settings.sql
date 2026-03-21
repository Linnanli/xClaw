-- 技能表
CREATE TABLE IF NOT EXISTS skills (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT DEFAULT '',
    version VARCHAR(50) NOT NULL DEFAULT '1.0.0',
    author VARCHAR(255) DEFAULT '',
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 插件表
CREATE TABLE IF NOT EXISTS plugins (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT DEFAULT '',
    version VARCHAR(50) NOT NULL DEFAULT '1.0.0',
    author VARCHAR(255) DEFAULT '',
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 系统配置表（key-value 存储）
CREATE TABLE IF NOT EXISTS system_settings (
    key VARCHAR(255) PRIMARY KEY,
    value JSONB NOT NULL DEFAULT '{}',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 插入默认系统配置
INSERT INTO system_settings (key, value) VALUES
    ('dlp_enabled', 'true'),
    ('dlp_scan_timeout_ms', '5000'),
    ('dlp_fail_open', 'false'),
    ('audit_retention_days', '90'),
    ('audit_enabled', 'true'),
    ('client_heartbeat_interval_s', '30'),
    ('client_offline_threshold_s', '120'),
    ('policy_sync_interval_s', '300'),
    ('policy_auto_push', 'true')
ON CONFLICT (key) DO NOTHING;
