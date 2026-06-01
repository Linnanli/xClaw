"""
场景 9 — chat-stream finish 必须落到当前 UI 会话。

回归背景：assistant-ui 在新会话发送时可能使用 `__LOCALID_*` 作为内部
chatId；后端会把真实 DB thread UUID 写入 `chat-stream` envelope。若前端把
`__LOCALID_*` 传给后端，后续真实 UUID 的 text / finish 帧会被
TauriChatTransport 按 threadId 过滤丢弃，表现为 UI 一直 loading。

本场景用真实 UI 点击发送，先确认底层 `chat-stream` 收到 finish，再确认 UI
出现新的助手气泡。若前端把 `__LOCALID_*` 传给后端，底层 finish 仍会出现，
但 TauriChatTransport 会因 threadId 不匹配丢弃帧，UI 不会结束 loading。
"""

from __future__ import annotations

from webdriver_client import (
    clear_chat_stream_events,
    click_send,
    is_e2e_capture_installed,
    poll,
    snapshot,
    type_into_composer,
    wait_chat_stream_finish,
    wait_composer_ready,
)


def test_short_message_finish_renders_assistant_reply(session_id):
    assert is_e2e_capture_installed(session_id), (
        "dev hook 未就绪——本场景要求 dev 构建启动："
        "`cargo tauri dev -f webdriver`"
    )

    s0 = wait_composer_ready(session_id, timeout=60.0)
    assert s0 is not None, "composer 未就绪"
    baseline = s0["assistantMsgs"]

    clear_chat_stream_events(session_id)
    type_into_composer(session_id, "你好")
    click_send(session_id)

    events = wait_chat_stream_finish(session_id, timeout=120.0)
    assert events is not None, (
        "120s 内未收到 finish 帧——模型未配置 / 网络不通 / "
        "tauri_channel.respond() 未发 finish"
    )

    def _has_new_reply():
        s = snapshot(session_id)
        return s if s["assistantMsgs"] > baseline else None

    s1 = poll(_has_new_reply, timeout=30.0, interval=1.0)
    assert s1 is not None, (
        "已收到 chat-stream finish，但 UI 未出现新助手气泡——可能是 threadId "
        "过滤丢弃了真实后端会话帧"
    )
