-- LSP server whitelist and Git repo whitelist settings (P1 Claude Code Parity)
-- Extends code_tool_settings with 'lsp_servers' and 'git_repos' setting types.

INSERT INTO code_tool_settings (department_id, setting_type, config) VALUES
    (NULL, 'lsp_servers', '{
        "allowed_servers": [],
        "note": "Empty list = allow all defaults (rust-analyzer, typescript-language-server, pyright, gopls). Non-empty = only allow listed servers."
    }'),
    (NULL, 'git_repos', '{
        "allowed_repos": [],
        "push_requires_approval": true,
        "commit_dlp_scan": true,
        "note": "Empty allowed_repos = allow any repo. Non-empty = restrict to listed repo URL patterns."
    }')
ON CONFLICT DO NOTHING;
