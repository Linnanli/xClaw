-- 客户端注册表
CREATE TABLE IF NOT EXISTS registered_clients (
    id UUID PRIMARY KEY,
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    username VARCHAR(255),
    client_name VARCHAR(255),
    version VARCHAR(50),
    os VARCHAR(100),
    ip_address VARCHAR(45),
    last_activity TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    online BOOLEAN NOT NULL DEFAULT false,
    policy_version VARCHAR(50),
    registered_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_clients_user_id ON registered_clients(user_id);
CREATE INDEX IF NOT EXISTS idx_clients_online ON registered_clients(online);
CREATE INDEX IF NOT EXISTS idx_clients_last_activity ON registered_clients(last_activity DESC);
