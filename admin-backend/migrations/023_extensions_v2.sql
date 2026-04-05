-- 023_extensions_v2.sql
-- 扩展管理重构：为 skills/plugins 表新增字段，创建部门技能白名单表

-- skills 表：补加 name 唯一约束（001_init.sql 遗漏）
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'skills_name_unique') THEN ALTER TABLE skills ADD CONSTRAINT skills_name_unique UNIQUE (name); END IF; END $$;

-- skills 表新增字段
ALTER TABLE skills
    ADD COLUMN IF NOT EXISTS source VARCHAR(20) NOT NULL DEFAULT 'builtin',
    ADD COLUMN IF NOT EXISTS review_status VARCHAR(20) NOT NULL DEFAULT 'approved',
    ADD COLUMN IF NOT EXISTS file_path TEXT,
    ADD COLUMN IF NOT EXISTS is_builtin BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN IF NOT EXISTS invoke_count BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS reviewed_by UUID REFERENCES users(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS reviewed_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS review_note TEXT,
    ADD COLUMN IF NOT EXISTS uploaded_by UUID REFERENCES users(id) ON DELETE SET NULL;

-- source: 'builtin' | 'admin_upload'
-- review_status: 'pending' | 'approved' | 'rejected'

-- plugins 表新增字段
ALTER TABLE plugins
    ADD COLUMN IF NOT EXISTS source VARCHAR(20) NOT NULL DEFAULT 'builtin',
    ADD COLUMN IF NOT EXISTS review_status VARCHAR(20) NOT NULL DEFAULT 'approved',
    ADD COLUMN IF NOT EXISTS plugin_type VARCHAR(20) NOT NULL DEFAULT 'http',
    ADD COLUMN IF NOT EXISTS file_path TEXT,
    ADD COLUMN IF NOT EXISTS is_builtin BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN IF NOT EXISTS invoke_count BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS requires_sandbox BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN IF NOT EXISTS reviewed_by UUID REFERENCES users(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS reviewed_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS review_note TEXT,
    ADD COLUMN IF NOT EXISTS uploaded_by UUID REFERENCES users(id) ON DELETE SET NULL;

-- plugin_type: 'http' | 'stdio' | 'wasm'

-- 部门技能白名单表
CREATE TABLE IF NOT EXISTS department_skill_whitelist (
    department_id UUID NOT NULL REFERENCES departments(id) ON DELETE CASCADE,
    skill_id      UUID NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (department_id, skill_id)
);

CREATE INDEX IF NOT EXISTS idx_dept_skill_whitelist_dept ON department_skill_whitelist(department_id);

-- ============================================================================
-- 内置技能种子数据（来自 ironclaw/skills/）
-- ON CONFLICT (id) 基于固定 UUID，幂等安全
-- ============================================================================

INSERT INTO skills (id, name, description, version, author, enabled, source, review_status, is_builtin, created_at, updated_at)
VALUES
    (
        'a1000000-0000-0000-0000-000000000001',
        'delegation',
        'Helps users delegate tasks, break them into steps, set deadlines, and track progress via routines and memory.',
        '0.1.0', 'ironclaw', true, 'builtin', 'approved', true, NOW(), NOW()
    ),
    (
        'a1000000-0000-0000-0000-000000000002',
        'review-checklist',
        'Pre-merge review checklist based on recurring AI reviewer feedback patterns.',
        '0.1.0', 'ironclaw', true, 'builtin', 'approved', true, NOW(), NOW()
    ),
    (
        'a1000000-0000-0000-0000-000000000003',
        'routine-advisor',
        'Suggests relevant cron routines based on user context, goals, and observed patterns.',
        '0.1.0', 'ironclaw', true, 'builtin', 'approved', true, NOW(), NOW()
    ),
    (
        'a1000000-0000-0000-0000-000000000004',
        'ironclaw-workflow-orchestrator',
        'Install and operate a full GitHub issue-to-merge workflow in IronClaw using event-driven and cron routines.',
        '1.0.0', 'ironclaw', true, 'builtin', 'approved', true, NOW(), NOW()
    )
ON CONFLICT (id) DO UPDATE SET
    description   = EXCLUDED.description,
    version       = EXCLUDED.version,
    source        = EXCLUDED.source,
    review_status = EXCLUDED.review_status,
    is_builtin    = EXCLUDED.is_builtin,
    updated_at    = NOW();

-- ============================================================================
-- 内置插件种子数据（来自 ironclaw/registry/）
-- MCP Servers → plugin_type='http'
-- WASM Tools  → plugin_type='wasm'
-- ============================================================================

INSERT INTO plugins (id, name, description, version, author, enabled, source, review_status, plugin_type, is_builtin, requires_sandbox, created_at, updated_at)
VALUES
    -- MCP Servers
    ('b1000000-0000-0000-0000-000000000001', 'notion',
     'Connect to Notion for reading and writing pages, databases, and comments.',
     '1.0.0', 'ironclaw', true, 'builtin', 'approved', 'http', true, false, NOW(), NOW()),
    ('b1000000-0000-0000-0000-000000000002', 'linear',
     'Linear integration for issue tracking and project management.',
     '1.0.0', 'ironclaw', true, 'builtin', 'approved', 'http', true, false, NOW(), NOW()),
    ('b1000000-0000-0000-0000-000000000003', 'stripe',
     'Stripe integration for payment and subscription management.',
     '1.0.0', 'ironclaw', true, 'builtin', 'approved', 'http', true, false, NOW(), NOW()),
    ('b1000000-0000-0000-0000-000000000004', 'sentry',
     'Sentry integration for error tracking and performance monitoring.',
     '1.0.0', 'ironclaw', true, 'builtin', 'approved', 'http', true, false, NOW(), NOW()),
    ('b1000000-0000-0000-0000-000000000005', 'cloudflare',
     'Cloudflare integration for DNS, Workers, and security management.',
     '1.0.0', 'ironclaw', true, 'builtin', 'approved', 'http', true, false, NOW(), NOW()),
    ('b1000000-0000-0000-0000-000000000006', 'intercom',
     'Intercom integration for customer messaging and support.',
     '1.0.0', 'ironclaw', true, 'builtin', 'approved', 'http', true, false, NOW(), NOW()),
    ('b1000000-0000-0000-0000-000000000007', 'asana',
     'Asana integration for project and task management.',
     '1.0.0', 'ironclaw', true, 'builtin', 'approved', 'http', true, false, NOW(), NOW()),
    -- WASM Tools
    ('b1000000-0000-0000-0000-000000000011', 'github',
     'GitHub integration for issues, PRs, repos, and code search.',
     '0.2.2', 'ironclaw', true, 'builtin', 'approved', 'wasm', true, false, NOW(), NOW()),
    ('b1000000-0000-0000-0000-000000000012', 'gmail',
     'Gmail integration for reading and sending emails.',
     '1.0.0', 'ironclaw', true, 'builtin', 'approved', 'wasm', true, false, NOW(), NOW()),
    ('b1000000-0000-0000-0000-000000000013', 'google-calendar',
     'Google Calendar integration for scheduling and event management.',
     '1.0.0', 'ironclaw', true, 'builtin', 'approved', 'wasm', true, false, NOW(), NOW()),
    ('b1000000-0000-0000-0000-000000000014', 'google-drive',
     'Google Drive integration for file storage and sharing.',
     '1.0.0', 'ironclaw', true, 'builtin', 'approved', 'wasm', true, false, NOW(), NOW()),
    ('b1000000-0000-0000-0000-000000000015', 'google-docs',
     'Google Docs integration for document creation and editing.',
     '1.0.0', 'ironclaw', true, 'builtin', 'approved', 'wasm', true, false, NOW(), NOW()),
    ('b1000000-0000-0000-0000-000000000016', 'google-sheets',
     'Google Sheets integration for spreadsheet management.',
     '1.0.0', 'ironclaw', true, 'builtin', 'approved', 'wasm', true, false, NOW(), NOW()),
    ('b1000000-0000-0000-0000-000000000017', 'google-slides',
     'Google Slides integration for presentation creation.',
     '1.0.0', 'ironclaw', true, 'builtin', 'approved', 'wasm', true, false, NOW(), NOW()),
    ('b1000000-0000-0000-0000-000000000018', 'web-search',
     'Web search integration for real-time information retrieval.',
     '1.0.0', 'ironclaw', true, 'builtin', 'approved', 'wasm', true, false, NOW(), NOW()),
    ('b1000000-0000-0000-0000-000000000019', 'slack',
     'Slack integration for team messaging and notifications.',
     '1.0.0', 'ironclaw', true, 'builtin', 'approved', 'wasm', true, false, NOW(), NOW())
ON CONFLICT (id) DO UPDATE SET
    description    = EXCLUDED.description,
    version        = EXCLUDED.version,
    source         = EXCLUDED.source,
    review_status  = EXCLUDED.review_status,
    plugin_type    = EXCLUDED.plugin_type,
    is_builtin     = EXCLUDED.is_builtin,
    updated_at     = NOW();
