-- 操作审批流
--
-- 变更内容：
-- 1. 新建 approval_tickets 审批工单表

CREATE TABLE IF NOT EXISTS approval_tickets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- 申请人
    applicant_id UUID NOT NULL REFERENCES users(id),
    -- 关联的敏感操作规则（可选，手动创建的工单可能不关联规则）
    operation_rule_id UUID REFERENCES sensitive_operation_rules(id) ON DELETE SET NULL,
    -- 操作描述（快照，规则删除后仍可追溯）
    operation_type VARCHAR(100) NOT NULL,
    operation_name VARCHAR(200) NOT NULL,
    -- 申请理由
    reason TEXT,
    -- 状态：pending / approved / rejected / expired
    status VARCHAR(20) NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'approved', 'rejected', 'expired')),
    -- 审批人
    reviewer_id UUID REFERENCES users(id) ON DELETE SET NULL,
    review_comment TEXT,
    reviewed_at TIMESTAMP WITH TIME ZONE,
    -- 有效期（批准后，申请人必须在此时间前执行操作）
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT (NOW() + INTERVAL '24 hours'),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_approval_tickets_status ON approval_tickets(status);
CREATE INDEX IF NOT EXISTS idx_approval_tickets_applicant ON approval_tickets(applicant_id);
CREATE INDEX IF NOT EXISTS idx_approval_tickets_created ON approval_tickets(created_at);
