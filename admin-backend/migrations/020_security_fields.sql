-- 安全加固扩展字段
--
-- 变更内容：
-- 1. users 表：MFA 支持 + 账户锁定
-- 2. audit_logs 表：IP 地址 + User-Agent + 不可变标记
-- 3. clients 表：设备指纹 + 版本升级标记
-- 4. model_configs 表：调用统计 + 连续失败计数 + 最后错误时间

-- 1. users 表扩展
ALTER TABLE users
    ADD COLUMN IF NOT EXISTS mfa_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS login_fail_count INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS locked_until TIMESTAMP WITH TIME ZONE;

-- 2. audit_logs 表扩展
ALTER TABLE audit_logs
    ADD COLUMN IF NOT EXISTS ip_address VARCHAR(45),
    ADD COLUMN IF NOT EXISTS user_agent TEXT,
    ADD COLUMN IF NOT EXISTS is_immutable BOOLEAN NOT NULL DEFAULT TRUE;

-- 3. clients 表扩展
ALTER TABLE registered_clients
    ADD COLUMN IF NOT EXISTS device_fingerprint JSONB,
    ADD COLUMN IF NOT EXISTS needs_upgrade BOOLEAN NOT NULL DEFAULT FALSE;

-- 4. model_configs 表扩展
ALTER TABLE model_configs
    ADD COLUMN IF NOT EXISTS total_calls BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS avg_latency_ms DOUBLE PRECISION,
    ADD COLUMN IF NOT EXISTS consecutive_failures INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS last_error_at TIMESTAMP WITH TIME ZONE;
