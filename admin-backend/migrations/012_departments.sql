-- 部门管理
-- 每个用户可归属一个部门，部门可配置 Token 限额

CREATE TABLE IF NOT EXISTS departments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL UNIQUE,
    description TEXT,
    token_quota_enabled BOOLEAN NOT NULL DEFAULT false,
    token_quota_per_day INTEGER,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_departments_name ON departments(name);

-- users 表新增 department_id 字段
ALTER TABLE users
    ADD COLUMN IF NOT EXISTS department_id UUID REFERENCES departments(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_users_department_id ON users(department_id);
