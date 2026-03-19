-- Roles table
CREATE TABLE IF NOT EXISTS roles (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL
);

-- Permissions table
CREATE TABLE IF NOT EXISTS permissions (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    resource VARCHAR(255) NOT NULL,
    action VARCHAR(255) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL
);

-- Role permissions (many-to-many)
CREATE TABLE IF NOT EXISTS role_permissions (
    role_id UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    permission_id UUID NOT NULL REFERENCES permissions(id) ON DELETE CASCADE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    PRIMARY KEY (role_id, permission_id)
);

-- User roles (many-to-many)
CREATE TABLE IF NOT EXISTS user_roles (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role_id UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    PRIMARY KEY (user_id, role_id)
);

-- Create indexes
CREATE INDEX idx_roles_name ON roles(name);
CREATE INDEX idx_permissions_resource ON permissions(resource);
CREATE INDEX idx_permissions_action ON permissions(action);
CREATE INDEX idx_role_permissions_role_id ON role_permissions(role_id);
CREATE INDEX idx_role_permissions_permission_id ON role_permissions(permission_id);
CREATE INDEX idx_user_roles_user_id ON user_roles(user_id);
CREATE INDEX idx_user_roles_role_id ON user_roles(role_id);

-- Insert default roles
INSERT INTO roles (id, name, description, created_at, updated_at) VALUES
    (gen_random_uuid(), 'admin', '系统管理员，拥有所有权限', NOW(), NOW()),
    (gen_random_uuid(), 'user', '普通用户，拥有基本权限', NOW(), NOW()),
    (gen_random_uuid(), 'auditor', '审计员，只能查看日志', NOW(), NOW());

-- Insert default permissions
INSERT INTO permissions (id, name, description, resource, action, created_at) VALUES
    -- 用户管理权限
    (gen_random_uuid(), 'users.list', '查看用户列表', 'users', 'list', NOW()),
    (gen_random_uuid(), 'users.create', '创建用户', 'users', 'create', NOW()),
    (gen_random_uuid(), 'users.update', '更新用户', 'users', 'update', NOW()),
    (gen_random_uuid(), 'users.delete', '删除用户', 'users', 'delete', NOW()),
    -- 角色管理权限
    (gen_random_uuid(), 'roles.list', '查看角色列表', 'roles', 'list', NOW()),
    (gen_random_uuid(), 'roles.create', '创建角色', 'roles', 'create', NOW()),
    (gen_random_uuid(), 'roles.update', '更新角色', 'roles', 'update', NOW()),
    (gen_random_uuid(), 'roles.delete', '删除角色', 'roles', 'delete', NOW()),
    -- 权限管理权限
    (gen_random_uuid(), 'permissions.list', '查看权限列表', 'permissions', 'list', NOW()),
    -- DLP 管理权限
    (gen_random_uuid(), 'dlp.list', '查看 DLP 规则', 'dlp', 'list', NOW()),
    (gen_random_uuid(), 'dlp.create', '创建 DLP 规则', 'dlp', 'create', NOW()),
    (gen_random_uuid(), 'dlp.update', '更新 DLP 规则', 'dlp', 'update', NOW()),
    (gen_random_uuid(), 'dlp.delete', '删除 DLP 规则', 'dlp', 'delete', NOW()),
    -- 审计日志权限
    (gen_random_uuid(), 'audit.list', '查看审计日志', 'audit', 'list', NOW()),
    (gen_random_uuid(), 'audit.export', '导出审计日志', 'audit', 'export', NOW());

-- Assign all permissions to admin role
INSERT INTO role_permissions (role_id, permission_id, created_at)
SELECT r.id, p.id, NOW()
FROM roles r
CROSS JOIN permissions p
WHERE r.name = 'admin';

-- Assign basic permissions to user role
INSERT INTO role_permissions (role_id, permission_id, created_at)
SELECT r.id, p.id, NOW()
FROM roles r
CROSS JOIN permissions p
WHERE r.name = 'user'
AND p.name IN ('users.list', 'dlp.list', 'audit.list');

-- Assign audit permissions to auditor role
INSERT INTO role_permissions (role_id, permission_id, created_at)
SELECT r.id, p.id, NOW()
FROM roles r
CROSS JOIN permissions p
WHERE r.name = 'auditor'
AND p.name IN ('audit.list', 'audit.export');

-- Assign admin role to existing admin user
INSERT INTO user_roles (user_id, role_id, created_at)
SELECT u.id, r.id, NOW()
FROM users u
CROSS JOIN roles r
WHERE u.username = 'admin'
AND r.name = 'admin'
ON CONFLICT DO NOTHING;
