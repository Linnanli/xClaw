"""
场景 9 — assistant-ui 本地 thread id 不应进入后端 send_chat_message。

回归背景：assistant-ui 在新会话发送时可能使用 `__LOCALID_*` 作为内部
chatId；后端会把真实 DB thread UUID 写入 `chat-stream` envelope。若前端把
`__LOCALID_*` 传给后端，后续真实 UUID 的 text / finish 帧会被
TauriChatTransport 按 threadId 过滤丢弃，表现为 UI 一直 loading。

本场景通过 DEV-only invoke 捕获 hook 验证：真实 UI 点击发送时，传给
`send_chat_message` 的 threadId 必须是后端真实 thread id，而不是
assistant-ui 本地 id。
"""

from __future__ import annotations

from webdriver_client import (
    clear_tauri_invokes,
    click_send,
    is_e2e_capture_installed,
    type_into_composer,
    wait_composer_ready,
    wait_tauri_invoke,
)


def test_send_message_uses_real_thread_id_for_backend_invoke(session_id):
    assert is_e2e_capture_installed(session_id), (
        "dev hook 未就绪——本场景要求 dev 构建启动："
        "`cargo tauri dev -f webdriver`"
    )

    s0 = wait_composer_ready(session_id, timeout=60.0)
    assert s0 is not None, "composer 未就绪"

    clear_tauri_invokes(session_id)
    type_into_composer(session_id, "你好")
    click_send(session_id)

    call = wait_tauri_invoke(session_id, "send_chat_message", timeout=30.0)
    assert call is not None, "30s 内未捕获 send_chat_message invoke"

    args = call.get("args") if isinstance(call, dict) else None
    assert isinstance(args, dict), f"send_chat_message args 格式异常：{args!r}"

    thread_id = args.get("threadId")
    assert isinstance(thread_id, str) and thread_id, (
        f"send_chat_message.threadId 必须是非空字符串，got {thread_id!r}"
    )
    assert not thread_id.startswith("__LOCALID_"), (
        "send_chat_message.threadId 不应使用 assistant-ui 本地 id，"
        f"否则真实 UUID 的 chat-stream finish 会被过滤：{thread_id}"
    )