-- 006: DLP 字典表 + 规则类型扩展支持 dictionary
-- 创建字典表
CREATE TABLE IF NOT EXISTS dlp_dictionaries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL UNIQUE,
    description TEXT,
    keywords TEXT[] NOT NULL DEFAULT '{}',
    keyword_count INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 创建字典名称索引
CREATE INDEX IF NOT EXISTS idx_dlp_dictionaries_name ON dlp_dictionaries(name);

-- 扩展 dlp_rules.rule_type 支持 'dictionary' 值（无需 ALTER，VARCHAR 已支持）
-- rule_config 中 dictionary 类型存储: {"dictionary_id": "uuid", "match_mode": "contains", "case_sensitive": false}
