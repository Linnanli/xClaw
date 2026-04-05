-- 客户端管理扩展
--
-- 变更内容：
-- 1. system_settings 新增 minimum_client_version 配置项
-- 2. registered_clients 表已有 needs_upgrade 和 device_fingerprint 字段（migration 020）

-- 新增最低客户端版本配置（空字符串表示不限制）
INSERT INTO system_settings (key, value) VALUES
    ('minimum_client_version', '""')
ON CONFLICT (key) DO NOTHING;
