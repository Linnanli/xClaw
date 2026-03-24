-- 模型配置表
-- 管理端配置可用的 AI 模型列表，下发给客户端
CREATE TABLE IF NOT EXISTS model_configs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- 模型标识（唯一，用于 API 调用）
    model_id VARCHAR(100) NOT NULL UNIQUE,
    -- 显示名称
    display_name VARCHAR(200) NOT NULL,
    -- 模型描述
    description TEXT,
    -- 模型提供商（openai / anthropic / deepseek / custom）
    provider VARCHAR(50) NOT NULL DEFAULT 'custom',
    -- API 基础 URL
    api_base_url TEXT,
    -- API Key（加密存储）
    api_key TEXT,
    -- 是否启用
    enabled BOOLEAN NOT NULL DEFAULT true,
    -- 是否为默认模型
    is_default BOOLEAN NOT NULL DEFAULT false,
    -- 排序权重（越小越靠前）
    sort_order INTEGER NOT NULL DEFAULT 100,
    -- 模型能力标签（JSON 数组，如 ["chat", "vision", "code"]）
    capabilities JSONB DEFAULT '[]'::jsonb,
    -- 额外配置（JSON，如 max_tokens, temperature 等）
    extra_config JSONB DEFAULT '{}'::jsonb,
    -- 创建者
    created_by UUID,
    -- 时间戳
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- 确保只有一个默认模型
CREATE UNIQUE INDEX IF NOT EXISTS idx_model_configs_default
    ON model_configs (is_default) WHERE is_default = true;

CREATE INDEX IF NOT EXISTS idx_model_configs_enabled
    ON model_configs (enabled);

CREATE INDEX IF NOT EXISTS idx_model_configs_sort
    ON model_configs (sort_order, display_name);

-- 插入默认模型配置
INSERT INTO model_configs (model_id, display_name, description, provider, is_default, sort_order)
VALUES
    ('gpt-4o', 'GPT-4o', '最强大的多模态模型', 'openai', true, 10),
    ('gpt-4o-mini', 'GPT-4o Mini', '快速且经济', 'openai', false, 20),
    ('claude-3.5-sonnet', 'Claude 3.5 Sonnet', 'Anthropic 旗舰模型', 'anthropic', false, 30),
    ('deepseek-r1', 'DeepSeek R1', '高性价比推理模型', 'deepseek', false, 40)
ON CONFLICT (model_id) DO NOTHING;
