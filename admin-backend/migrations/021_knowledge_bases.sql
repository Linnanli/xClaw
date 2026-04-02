-- 知识库管理
--
-- 变更内容：
-- 1. 新建 knowledge_bases 知识库表
-- 2. 新建 kb_documents 知识库文档表

-- 1. 知识库
CREATE TABLE IF NOT EXISTS knowledge_bases (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(200) NOT NULL,
    description TEXT,
    document_count INTEGER NOT NULL DEFAULT 0,
    enabled BOOLEAN NOT NULL DEFAULT true,
    -- 允许访问的部门 ID 列表（JSON 数组），空数组表示不限制
    allowed_departments JSONB NOT NULL DEFAULT '[]',
    -- 允许访问的角色 ID 列表（JSON 数组），空数组表示不限制
    allowed_roles JSONB NOT NULL DEFAULT '[]',
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_knowledge_bases_enabled ON knowledge_bases(enabled);

-- 2. 知识库文档
CREATE TABLE IF NOT EXISTS kb_documents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    knowledge_base_id UUID NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    filename VARCHAR(500) NOT NULL,
    -- 文件类型：pdf / docx / md / txt
    file_type VARCHAR(20) NOT NULL,
    file_size BIGINT NOT NULL DEFAULT 0,
    -- 处理状态：pending / processing / completed / failed
    status VARCHAR(20) NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'processing', 'completed', 'failed')),
    chunk_count INTEGER NOT NULL DEFAULT 0,
    error_message TEXT,
    -- 文件存储路径
    storage_path TEXT,
    uploaded_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    processed_at TIMESTAMP WITH TIME ZONE
);

CREATE INDEX IF NOT EXISTS idx_kb_documents_kb ON kb_documents(knowledge_base_id);
CREATE INDEX IF NOT EXISTS idx_kb_documents_status ON kb_documents(status);
