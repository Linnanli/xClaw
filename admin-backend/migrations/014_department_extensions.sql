-- 部门管理扩展：树形架构 + 模型白名单
--
-- 变更内容：
-- 1. departments 表增加 parent_id 支持树形层级
-- 2. 新建 department_model_whitelist 关联表

-- 1. 树形架构支持
ALTER TABLE departments
    ADD COLUMN IF NOT EXISTS parent_id UUID REFERENCES departments(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_departments_parent_id ON departments(parent_id);

-- 2. 部门模型白名单
CREATE TABLE IF NOT EXISTS department_model_whitelist (
    department_id UUID NOT NULL REFERENCES departments(id) ON DELETE CASCADE,
    model_config_id UUID NOT NULL REFERENCES model_configs(id) ON DELETE CASCADE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    PRIMARY KEY (department_id, model_config_id)
);

CREATE INDEX IF NOT EXISTS idx_dept_model_wl_dept ON department_model_whitelist(department_id);
CREATE INDEX IF NOT EXISTS idx_dept_model_wl_model ON department_model_whitelist(model_config_id);
