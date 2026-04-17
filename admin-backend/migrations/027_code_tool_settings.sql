-- Code tool settings for Claude Code parity (P0)
-- Stores code-tools enable/disable, workspace path whitelist, and bash rules per department

CREATE TABLE IF NOT EXISTS code_tool_settings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    department_id UUID REFERENCES departments(id) ON DELETE CASCADE,
    -- NULL department_id = global default
    setting_type TEXT NOT NULL CHECK (setting_type IN ('code_tools', 'workspace_paths', 'bash_rules')),
    config JSONB NOT NULL DEFAULT '{}',
    updated_by UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (department_id, setting_type)
);

-- Index for fast lookup by department + type
CREATE INDEX IF NOT EXISTS idx_code_tool_settings_dept_type
    ON code_tool_settings(department_id, setting_type);

-- Insert global defaults
INSERT INTO code_tool_settings (department_id, setting_type, config) VALUES
    (NULL, 'code_tools', '{
        "enabled": true,
        "tools": {
            "code_edit": {"enabled": true, "require_approval": true},
            "grep_search": {"enabled": true, "require_approval": false},
            "glob_search": {"enabled": true, "require_approval": false},
            "read_file": {"enabled": true, "require_approval": false},
            "shell": {"enabled": true, "require_approval": true}
        }
    }'),
    (NULL, 'workspace_paths', '{
        "allowed_paths": [],
        "denied_paths": ["/etc", "/usr", "/var", "/root", "/home"],
        "enforce_workspace_boundary": true
    }'),
    (NULL, 'bash_rules', '{
        "custom_rules": [],
        "blocked_commands": ["rm -rf /", "mkfs", "dd if=/dev/zero"],
        "auto_approve_read_only": true,
        "sandbox_required_for_high_risk": true
    }')
ON CONFLICT DO NOTHING;
