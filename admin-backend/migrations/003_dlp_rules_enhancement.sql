-- 增强 DLP 规则表
-- 添加缺失的字段

-- 添加 name 字段
ALTER TABLE dlp_rules ADD COLUMN IF NOT EXISTS name VARCHAR(100);

-- 添加 description 字段
ALTER TABLE dlp_rules ADD COLUMN IF NOT EXISTS description TEXT;

-- 添加 enabled 字段
ALTER TABLE dlp_rules ADD COLUMN IF NOT EXISTS enabled BOOLEAN DEFAULT true;

-- 添加 category 字段
ALTER TABLE dlp_rules ADD COLUMN IF NOT EXISTS category VARCHAR(50) DEFAULT 'general';

-- 添加 updated_at 字段
ALTER TABLE dlp_rules ADD COLUMN IF NOT EXISTS updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP;

-- 为现有记录设置默认值
UPDATE dlp_rules SET name = 'Rule ' || id::text WHERE name IS NULL;
UPDATE dlp_rules SET category = 'general' WHERE category IS NULL;
UPDATE dlp_rules SET enabled = true WHERE enabled IS NULL;
UPDATE dlp_rules SET updated_at = created_at WHERE updated_at IS NULL;

-- 设置 name 为非空
ALTER TABLE dlp_rules ALTER COLUMN name SET NOT NULL;
