-- AI 对话审计
--
-- 变更内容：
-- 1. 新建 conversations 对话记录表
-- 2. 新建 conversation_messages 对话消息表

-- 1. 对话记录（每轮对话一条）
CREATE TABLE IF NOT EXISTS conversations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- 客户端生成的唯一 ID，用于幂等去重
    client_conversation_id VARCHAR(200) UNIQUE,
    user_id UUID NOT NULL REFERENCES users(id),
    topic VARCHAR(500) NOT NULL DEFAULT '',
    model_id VARCHAR(100),
    message_count INTEGER NOT NULL DEFAULT 0,
    total_tokens INTEGER NOT NULL DEFAULT 0,
    dlp_flagged BOOLEAN NOT NULL DEFAULT false,
    dlp_details TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_conversations_user ON conversations(user_id);
CREATE INDEX IF NOT EXISTS idx_conversations_created ON conversations(created_at);
CREATE INDEX IF NOT EXISTS idx_conversations_dlp ON conversations(dlp_flagged);

-- 2. 对话消息（每条消息一条）
CREATE TABLE IF NOT EXISTS conversation_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    conversation_id UUID NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    role VARCHAR(20) NOT NULL CHECK (role IN ('user', 'assistant', 'system')),
    content TEXT NOT NULL,
    model_id VARCHAR(100),
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_conv_messages_conv ON conversation_messages(conversation_id);
