-- 添加 DLP 规则类型支持（关键字匹配、字典匹配等）
-- rule_type: 'regex'（默认）, 'keyword'
-- rule_config: JSON 格式的额外配置

ALTER TABLE dlp_rules ADD COLUMN IF NOT EXISTS rule_type VARCHAR(20) NOT NULL DEFAULT 'regex';
ALTER TABLE dlp_rules ADD COLUMN IF NOT EXISTS rule_config JSONB;

-- 为现有规则设置默认值
UPDATE dlp_rules SET rule_type = 'regex' WHERE rule_type IS NULL;

-- 添加索引
CREATE INDEX IF NOT EXISTS idx_dlp_rules_rule_type ON dlp_rules(rule_type);

COMMENT ON COLUMN dlp_rules.rule_type IS '规则类型: regex=正则表达式, keyword=关键字匹配';
COMMENT ON COLUMN dlp_rules.rule_config IS '规则额外配置(JSON): keyword类型包含 keywords[], match_mode, case_sensitive';
