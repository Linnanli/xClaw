"""
场景 5 — DLP 出站脱敏与拦截（Fail-Safe）

对应：desktop-client/docs/e2e-webdriver-test-plan.md §5

验证 SafetyBridge 在 Tauri IPC / send_chat_message 层的语义：
  (a) PII（邮箱）→ had_sensitive_data=true 且原文不在 sanitized_content
  (b) 密钥样式串 → was_blocked=true（强制拦截，Fail-Safe）
  (c) 存储脱敏 sanitize_for_storage → 返回字符串中无原文敏感串
    (d) 普通脱敏消息的 SafetyBridge 日志早于 agent dispatch 日志
    (e) blocked 消息有 block 日志，且同一线程窗口无 dispatch 日志

为什么不跑 send_chat_message UI 路径：
    UI composer 路径拿不到 send_chat_message 返回的 message_id，无法按同一条消息关联日志。
    日志序用例直接调 send_chat_message IPC，保留真实 SafetyBridge → agent dispatch 顺序，
    同时避开 DOM/LLM 回复时序对审计日志断言的干扰。
"""

from __future__ import annotations

import uuid

from webdriver_client import clear_logs, invoke, poll, search_logs

# 经典 PII：邮箱地址（落入 DLP PII 规则）
EMAIL_SAMPLE = "请把账单发送到 alice.smith@example.com 这个邮箱。"

# 密钥样式串：OpenAI 风格 `sk-` 前缀长 token，应被 leak_detector 拦截
SECRET_SAMPLE = "我的 API key 是 sk-DEADBEEFCAFEBABE0123456789abcdef0123456789ABCDEF"

LOG_ORDER_TIMEOUT = 30.0
LOG_LEVEL_HINT = (
    "请确认启动前设置 "
    "RUST_LOG=desktop_client=debug,ironclaw=debug,tower_http=warn"
)


def _new_thread_id(session_id: str) -> str:
    thread_id = invoke(session_id, "ic_create_thread", {})
    assert isinstance(thread_id, str) and thread_id, (
        f"ic_create_thread 返回异常：{thread_id!r}"
    )
    return thread_id


def _send_chat_message(session_id: str, thread_id: str, content: str) -> dict:
    resp = invoke(
        session_id,
        "send_chat_message",
        {
            "threadId": thread_id,
            "content": content,
            "modelId": None,
            "apiBaseUrl": None,
            "apiKey": None,
            "dlpStats": None,
            "attachments": None,
        },
    )
    assert isinstance(resp, dict), f"send_chat_message 返回非 dict：{resp!r}"
    assert isinstance(resp.get("message_id"), str), f"缺少 message_id：{resp!r}"
    return resp


def _logs_for_message(session_id: str, query: str, message_id: str) -> list[dict]:
    return [
        entry
        for entry in search_logs(session_id, query, limit=200)
        if message_id in entry.get("message", "")
    ]


def _logs_for_thread(session_id: str, query: str, thread_id: str) -> list[dict]:
    return [
        entry
        for entry in search_logs(session_id, query, limit=200)
        if thread_id in entry.get("message", "")
    ]


def _wait_logs_for_message(
    session_id: str,
    query: str,
    message_id: str,
) -> list[dict] | None:
    return poll(
        lambda: _logs_for_message(session_id, query, message_id) or None,
        timeout=LOG_ORDER_TIMEOUT,
        interval=0.5,
    )


def _wait_logs_for_thread(
    session_id: str,
    query: str,
    thread_id: str,
) -> list[dict] | None:
    return poll(
        lambda: _logs_for_thread(session_id, query, thread_id) or None,
        timeout=LOG_ORDER_TIMEOUT,
        interval=0.5,
    )


def _log_index(logs: list[dict], needle: str, message_id: str) -> int:
    for idx, entry in enumerate(logs):
        message = entry.get("message", "")
        if needle in message and message_id in message:
            return idx
    raise AssertionError(f"未找到日志 {needle!r} message_id={message_id}: {logs!r}")


def test_scan_user_input_redacts_email(session_id):
    """(a) 邮箱 → had_sensitive_data 且 sanitized_content 不含原文。"""
    resp = invoke(session_id, "scan_user_input", {"content": EMAIL_SAMPLE})

    assert isinstance(resp, dict), f"返回非 dict：{resp!r}"
    assert resp["had_sensitive_data"] is True, (
        f"邮箱未被识别为敏感数据，DLP 规则可能未加载：{resp!r}"
    )
    assert "alice.smith@example.com" not in resp["sanitized_content"], (
        "Fail-Open：原文邮箱出现在 sanitized_content 中——脱敏未生效。"
        f" sanitized_content={resp['sanitized_content']!r}"
    )


def test_scan_user_input_blocks_secret(session_id):
    """(b) 密钥样式串 → was_blocked=true 或 blocked_count >= 1。

    leak_detector 对 `sk-` 前缀长 token 默认采取 BLOCK 策略（见
    crates/dasclaw_safety/src/leak_detector.rs L356+），was_blocked 必须为真；
    若策略降级为 redact 也至少要计入 blocked_count（兼容未来策略调整）。
    """
    resp = invoke(session_id, "scan_user_input", {"content": SECRET_SAMPLE})

    assert isinstance(resp, dict), f"返回非 dict：{resp!r}"
    blocked = bool(resp.get("was_blocked")) or (
        resp.get("sanitization_stats", {}).get("blocked_count", 0) >= 1
    )
    assert blocked, (
        "⚠️ Fail-Open：sk- 前缀密钥既未被拦截也未计入 blocked_count——"
        f"安全回归。 resp={resp!r}"
    )
    assert "sk-DEADBEEFCAFEBABE0123456789abcdef0123456789ABCDEF" not in resp.get(
        "sanitized_content", ""
    ), f"原文密钥泄露到 sanitized_content：{resp['sanitized_content']!r}"


def test_sanitize_for_storage_drops_pii(session_id):
    """(c) 存储脱敏：sanitize_for_storage 返回字符串不含原文敏感串。"""
    raw = "用户邮箱 bob@corp.com 已记录到审计日志"
    resp = invoke(session_id, "sanitize_for_storage", {"content": raw})

    assert isinstance(resp, str), f"sanitize_for_storage 返回非 string：{resp!r}"
    assert "bob@corp.com" not in resp, (
        "存储脱敏未生效——原文邮箱可能落入持久化存储造成长期泄露。"
        f" returned={resp!r}"
    )


def test_send_chat_message_dlp_sanitize_log_precedes_agent_dispatch(session_id):
    """(d) 同一 message_id 下，DLP 脱敏日志必须早于 agent dispatch 日志。"""
    clear_logs(session_id)
    thread_id = _new_thread_id(session_id)
    marker = f"e2e-log-order-{uuid.uuid4().hex[:8]}"

    resp = _send_chat_message(session_id, thread_id, f"{marker} {EMAIL_SAMPLE}")
    message_id = resp["message_id"]

    sanitized = _wait_logs_for_message(
        session_id, "Message sanitized by SafetyBridge", message_id
    )
    injected = _wait_logs_for_message(
        session_id, "Message injected into agent loop", message_id
    )
    assert sanitized, (
        f"未找到 SafetyBridge 脱敏日志 message_id={message_id}。{LOG_LEVEL_HINT}"
    )
    assert injected, (
        f"未找到 agent dispatch 日志 message_id={message_id}。{LOG_LEVEL_HINT}"
    )

    scan_end = _wait_logs_for_message(
        session_id, "send_chat_message.scan_user_input.end", message_id
    )
    assert scan_end, (
        f"未找到 scan_user_input.end 日志 message_id={message_id}。{LOG_LEVEL_HINT}"
    )

    relevant_logs = search_logs(session_id, message_id, limit=200)
    scan_idx = _log_index(
        relevant_logs, "send_chat_message.scan_user_input.end", message_id
    )
    sanitized_idx = _log_index(
        relevant_logs, "Message sanitized by SafetyBridge", message_id
    )
    injected_idx = _log_index(
        relevant_logs, "Message injected into agent loop", message_id
    )

    assert scan_idx <= sanitized_idx < injected_idx, (
        "DLP 脱敏/扫描日志必须早于 agent dispatch 日志："
        f"scan_idx={scan_idx}, sanitized_idx={sanitized_idx}, injected_idx={injected_idx}, "
        f"message_id={message_id}, logs={relevant_logs!r}"
    )


def test_send_chat_message_blocked_log_has_no_agent_dispatch(session_id):
    """(e) blocked 分支：同一 thread_id 窗口内有 block 日志，且无 dispatch 日志。"""
    clear_logs(session_id)
    thread_id = _new_thread_id(session_id)

    try:
        _send_chat_message(session_id, thread_id, SECRET_SAMPLE)
    except RuntimeError:
        pass
    else:
        raise AssertionError("Fail-Open：密钥样式消息未被 send_chat_message 拦截")

    blocked = _wait_logs_for_thread(
        session_id, "Message blocked by SafetyBridge", thread_id
    )
    assert blocked, f"未找到 blocked 日志 thread_id={thread_id}。{LOG_LEVEL_HINT}"

    injected = _logs_for_thread(session_id, "Message injected into agent loop", thread_id)
    assert not injected, (
        "Fail-Open：blocked 消息同一 thread_id 下仍出现 agent dispatch 日志。"
        f" thread_id={thread_id}, injected={injected!r}"
    )
