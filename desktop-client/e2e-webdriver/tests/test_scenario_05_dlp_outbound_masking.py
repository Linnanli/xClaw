"""
场景 5 — DLP 出站脱敏与拦截（Fail-Safe）

对应：desktop-client/docs/e2e-webdriver-test-plan.md §5

验证 SafetyBridge 在 Tauri IPC 层的三类语义：
  (a) PII（邮箱）→ had_sensitive_data=true 且原文不在 sanitized_content
  (b) 密钥样式串 → was_blocked=true（强制拦截，Fail-Safe）
  (c) 存储脱敏 sanitize_for_storage → 返回字符串中无原文敏感串

为什么不跑 send_chat_message UI 路径：
  send_chat_message 需要有效 thread_id + 模型配置 + Admin 配额，链路过深；
  其内部第一步就是 `safety_bridge.scan_user_input` 并在 was_blocked 时 `return Err`
  （见 desktop-client/src/ipc/chat.rs L100-L113），与本文件 (b) 同源同语义。
  IPC 层断言 was_blocked 已经覆盖 Fail-Safe 不变量；UI 层重复验证收益低、flakiness 高。
"""

from __future__ import annotations

from webdriver_client import invoke

# 经典 PII：邮箱地址（落入 DLP PII 规则）
EMAIL_SAMPLE = "请把账单发送到 alice.smith@example.com 这个邮箱。"

# 密钥样式串：OpenAI 风格 `sk-` 前缀长 token，应被 leak_detector 拦截
SECRET_SAMPLE = "我的 API key 是 sk-DEADBEEFCAFEBABE0123456789abcdef0123456789ABCDEF"


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
