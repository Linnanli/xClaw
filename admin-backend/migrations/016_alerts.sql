-- 告警与通知系统
--
-- 变更内容：
-- 1. 新建 alert_rules 告警规则表
-- 2. 新建 alert_events 告警事件表
-- 3. 新建 notification_logs 通知发送日志表

-- 1. 告警规则
CREATE TABLE IF NOT EXISTS alert_rules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(200) NOT NULL,
    description TEXT,
    -- 事件类型：dlp_violation, quota_exceeded, model_error, abnormal_login, approval_timeout
    event_type VARCHAR(50) NOT NULL,
    -- 触发条件（JSON）：如 {"threshold": 10, "window_minutes": 60}
    condition JSONB NOT NULL DEFAULT '{}',
    severity VARCHAR(20) NOT NULL CHECK (severity IN ('low', 'medium', 'high', 'critical')),
    -- 通知渠道（JSON 数组）：如 ["email", "wecom", "dingtalk", "feishu"]
    notify_channels JSONB NOT NULL DEFAULT '[]',
    -- 静默期（分钟），同一规则在此期间内仅发送一次通知
    silence_minutes INTEGER NOT NULL DEFAULT 60,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_alert_rules_event_type ON alert_rules(event_type);
CREATE INDEX IF NOT EXISTS idx_alert_rules_enabled ON alert_rules(enabled);

-- 2. 告警事件
CREATE TABLE IF NOT EXISTS alert_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rule_id UUID REFERENCES alert_rules(id) ON DELETE SET NULL,
    -- 快照规则名称，规则删除后仍可追溯
    rule_name VARCHAR(200) NOT NULL,
    event_type VARCHAR(50) NOT NULL,
    severity VARCHAR(20) NOT NULL CHECK (severity IN ('low', 'medium', 'high', 'critical')),
    -- 触发详情（人类可读描述）
    trigger_detail TEXT NOT NULL,
    -- 原始事件数据（JSON），供排查用
    event_data JSONB,
    -- 处理状态
    status VARCHAR(20) NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'acknowledged', 'in_progress', 'closed')),
    -- 处理备注
    resolved_note TEXT,
    resolved_by UUID REFERENCES users(id) ON DELETE SET NULL,
    resolved_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_alert_events_status ON alert_events(status);
CREATE INDEX IF NOT EXISTS idx_alert_events_severity ON alert_events(severity);
CREATE INDEX IF NOT EXISTS idx_alert_events_created ON alert_events(created_at);
CREATE INDEX IF NOT EXISTS idx_alert_events_rule ON alert_events(rule_id);

-- 3. 通知发送日志
CREATE TABLE IF NOT EXISTS notification_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    alert_event_id UUID NOT NULL REFERENCES alert_events(id) ON DELETE CASCADE,
    channel VARCHAR(20) NOT NULL,
    -- pending / success / failed
    status VARCHAR(20) NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'success', 'failed')),
    attempts INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_notification_logs_event ON notification_logs(alert_event_id);
CREATE INDEX IF NOT EXISTS idx_notification_logs_status ON notification_logs(status);
