//! Proptest 数据生成器 - 用于属性测试的随机数据生成
//!
//! 提供各种请求和响应类型的生成器，用于属性测试中生成随机但有效的测试数据。

use proptest::prelude::*;
use desktop_client::api_client::{
    SendMessageRequest, ThreadInfo, Message, MemoryNode, MemoryContent,
    JobInfo, LogEntry, ApprovalRequest,
};

/// 生成随机消息内容（10-200 个字符）
pub fn arb_message_content() -> impl Strategy<Value = String> {
    r"[A-Za-z0-9 .,!?-]{10,200}"
}

/// 生成随机 thread_id（格式：thread-{8个小写字母数字}）
pub fn arb_thread_id() -> impl Strategy<Value = String> {
    "thread-[a-z0-9]{8}"
}

/// 生成随机 message_id（格式：msg-{8个小写字母数字}）
pub fn arb_message_id() -> impl Strategy<Value = String> {
    "msg-[a-z0-9]{8}"
}

/// 生成随机 job_id（格式：job-{8个小写字母数字}）
pub fn arb_job_id() -> impl Strategy<Value = String> {
    "job-[a-z0-9]{8}"
}

/// 生成随机 memory_id（格式：mem-{8个小写字母数字}）
pub fn arb_memory_id() -> impl Strategy<Value = String> {
    "mem-[a-z0-9]{8}"
}

/// 生成随机 RFC3339 时间戳
pub fn arb_timestamp() -> impl Strategy<Value = String> {
    r"2024-0[1-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9]Z"
}

/// 生成随机 SendMessageRequest
pub fn arb_send_message_request() -> impl Strategy<Value = SendMessageRequest> {
    (
        arb_message_content(),
        prop::option::of(arb_thread_id()),
    )
        .prop_map(|(content, thread_id)| SendMessageRequest {
            content,
            thread_id,
        })
}

/// 生成随机 ThreadInfo
pub fn arb_thread_info() -> impl Strategy<Value = ThreadInfo> {
    (
        arb_thread_id(),
        r"(Idle|Running|Paused)",
        0usize..100,
        arb_timestamp(),
        arb_timestamp(),
        prop::option::of(r"[A-Za-z0-9 ]{5,50}"),
        prop::option::of(r"(assistant|thread|channel)"),
        prop::option::of(r"(gateway|slack|discord)"),
    )
        .prop_map(
            |(id, state, turn_count, created_at, updated_at, title, thread_type, channel)| {
                ThreadInfo {
                    id,
                    state: state.to_string(),
                    turn_count,
                    created_at,
                    updated_at,
                    title,
                    thread_type,
                    channel,
                }
            },
        )
}

/// 生成随机 Message
pub fn arb_message() -> impl Strategy<Value = Message> {
    (
        arb_message_id(),
        arb_thread_id(),
        r"(user|assistant)",
        arb_message_content(),
        arb_timestamp(),
    )
        .prop_map(|(id, thread_id, role, content, created_at)| Message {
            id,
            thread_id,
            role: role.to_string(),
            content,
            created_at,
        })
}

/// 生成随机 MemoryNode
pub fn arb_memory_node() -> impl Strategy<Value = MemoryNode> {
    (
        arb_memory_id(),
        r"[A-Za-z0-9_-]{3,20}",
        r"(file|directory|document)",
    )
        .prop_map(|(id, name, node_type)| MemoryNode {
            id,
            name: name.to_string(),
            node_type: node_type.to_string(),
            children: Vec::new(),
            metadata: None,
        })
}

/// 生成随机 MemoryContent
pub fn arb_memory_content() -> impl Strategy<Value = MemoryContent> {
    (
        arb_memory_id(),
        r"[A-Za-z0-9_-]{3,20}",
        r"[A-Za-z0-9 .,!?-]{20,200}",
        arb_timestamp(),
    )
        .prop_map(|(id, name, content, updated_at)| MemoryContent {
            id,
            name: name.to_string(),
            content,
            updated_at,
        })
}

/// 生成随机 JobInfo
pub fn arb_job_info() -> impl Strategy<Value = JobInfo> {
    (
        arb_job_id(),
        r"(pending|in_progress|completed|failed)",
        arb_timestamp(),
        arb_timestamp(),
        prop::option::of(r"[A-Za-z0-9 ]{5,50}"),
    )
        .prop_map(|(id, status, created_at, updated_at, title)| JobInfo {
            id,
            status: status.to_string(),
            created_at,
            updated_at,
            title,
        })
}

/// 生成随机 LogEntry
pub fn arb_log_entry() -> impl Strategy<Value = LogEntry> {
    (
        arb_timestamp(),
        r"(DEBUG|INFO|WARN|ERROR)",
        r"[a-z_]{3,20}",
        r"[A-Za-z0-9 .,!?-]{10,100}",
    )
        .prop_map(|(timestamp, level, module, message)| LogEntry {
            timestamp,
            level: level.to_string(),
            module: module.to_string(),
            message,
            context: None,
        })
}

/// 生成随机 ApprovalRequest
pub fn arb_approval_request() -> impl Strategy<Value = ApprovalRequest> {
    (
        r"req-[a-z0-9]{8}",
        r"(approve|deny)",
        prop::option::of(arb_thread_id()),
    )
        .prop_map(|(request_id, action, thread_id)| ApprovalRequest {
            request_id: request_id.to_string(),
            action: action.to_string(),
            thread_id,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::test_runner::TestRunner;

    #[test]
    fn test_message_content_generator() {
        let mut runner = TestRunner::default();
        runner
            .run(&arb_message_content(), |content: String| {
                assert!(!content.is_empty());
                assert!(content.len() >= 10 && content.len() <= 200);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn test_thread_id_generator() {
        let mut runner = TestRunner::default();
        runner
            .run(&arb_thread_id(), |id: String| {
                assert!(id.starts_with("thread-"));
                assert_eq!(id.len(), 15); // "thread-" (7) + 8 chars = 15
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn test_send_message_request_generator() {
        let mut runner = TestRunner::default();
        runner
            .run(&arb_send_message_request(), |req: SendMessageRequest| {
                assert!(!req.content.is_empty());
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn test_thread_info_generator() {
        let mut runner = TestRunner::default();
        runner
            .run(&arb_thread_info(), |thread: ThreadInfo| {
                assert!(thread.id.starts_with("thread-"));
                assert!(thread.turn_count < 100);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn test_message_generator() {
        let mut runner = TestRunner::default();
        runner
            .run(&arb_message(), |msg: Message| {
                assert!(msg.id.starts_with("msg-"));
                assert!(msg.thread_id.starts_with("thread-"));
                assert!(!msg.content.is_empty());
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn test_job_info_generator() {
        let mut runner = TestRunner::default();
        runner
            .run(&arb_job_info(), |job: JobInfo| {
                assert!(job.id.starts_with("job-"));
                assert!(!job.status.is_empty());
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn test_log_entry_generator() {
        let mut runner = TestRunner::default();
        runner
            .run(&arb_log_entry(), |log: LogEntry| {
                assert!(!log.timestamp.is_empty());
                assert!(!log.level.is_empty());
                assert!(!log.module.is_empty());
                assert!(!log.message.is_empty());
                Ok(())
            })
            .unwrap();
    }
}
