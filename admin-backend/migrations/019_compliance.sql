-- 数据分类分级与合规
--
-- 变更内容：
-- 1. DLP 规则表增加 classification_level 字段
-- 2. 新建 compliance_reports 合规报告表
-- 3. 新建 data_retention_policies 数据保留策略表

-- 1. DLP 规则关联数据分级
ALTER TABLE dlp_rules
    ADD COLUMN IF NOT EXISTS classification_level VARCHAR(20)
        CHECK (classification_level IN ('public', 'internal', 'confidential', 'top_secret'));

-- 2. 合规审计报告
CREATE TABLE IF NOT EXISTS compliance_reports (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(200) NOT NULL,
    -- monthly / quarterly / annual / custom
    report_type VARCHAR(20) NOT NULL DEFAULT 'custom',
    start_date DATE NOT NULL,
    end_date DATE NOT NULL,
    -- 报告内容（JSON：安全事件汇总、DLP 统计、策略变更）
    content JSONB NOT NULL DEFAULT '{}',
    generated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_compliance_reports_created ON compliance_reports(created_at);

-- 3. 数据保留策略（按分级配置不同保留期限）
CREATE TABLE IF NOT EXISTS data_retention_policies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    classification_level VARCHAR(20) NOT NULL UNIQUE
        CHECK (classification_level IN ('public', 'internal', 'confidential', 'top_secret')),
    retention_days INTEGER NOT NULL DEFAULT 365,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- 预置四个默认保留策略
INSERT INTO data_retention_policies (classification_level, retention_days) VALUES
    ('public', 90),
    ('internal', 180),
    ('confidential', 365),
    ('top_secret', 730)
ON CONFLICT (classification_level) DO NOTHING;
