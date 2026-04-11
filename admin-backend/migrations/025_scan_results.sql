-- 025_scan_results.sql
-- 扩展安全扫描结果表：技能与插件共用

CREATE TABLE IF NOT EXISTS scan_results (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    target_type VARCHAR(10) NOT NULL,
    target_id UUID NOT NULL,
    scanner_type VARCHAR(50) NOT NULL,
    verdict VARCHAR(20) NOT NULL,
    is_safe BOOLEAN NOT NULL DEFAULT false,
    max_severity VARCHAR(10),
    findings_count INT NOT NULL DEFAULT 0,
    findings JSONB NOT NULL DEFAULT '[]',
    scan_duration_ms INT,
    scanned_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_scan_results_target
    ON scan_results(target_type, target_id);