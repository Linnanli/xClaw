-- 允许 audit_logs.user_id 为 NULL，以支持系统操作和客户端上报
-- 同时去掉外键约束，因为客户端上报的事件可能没有对应的 admin 用户

ALTER TABLE audit_logs DROP CONSTRAINT IF EXISTS audit_logs_user_id_fkey;
ALTER TABLE audit_logs ALTER COLUMN user_id DROP NOT NULL;
