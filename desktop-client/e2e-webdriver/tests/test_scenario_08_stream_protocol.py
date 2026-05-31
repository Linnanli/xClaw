"""
场景 8 — chat-stream 协议帧序验证

对应：issue #1035 / PR #1036。

验证：发一条能触发 thinking 的长提示词后，捕获真链路上的全部
`chat-stream` 帧，断言：
  1. 流必须以 `finish` 帧收尾（turn 真正结束）
  2. 至少出现一段完整 reasoning 生命周期（require_reasoning=True）
  3. 所有 reasoning-* 帧 id 非空、成对闭合（Start → Delta* → End）

这是 PR #1036 引入的 ReasoningSessions lifecycle 的回归哨兵。修复前
`map_status_to_stream` 对 `StatusUpdate::Thinking` 始终产出
`ReasoningDelta { id: "" }`，Vercel AI SDK v5 严格校验会抛
'Received reasoning-delta for missing reasoning part with ID ""'。

dev hook `window.__E2E_GET_EVENTS` 在 main.tsx 的 `import.meta.env.DEV`
分支自动安装。release bundle 不暴露该 hook——若 hook 不可达则 assert
失败而非 skip，避免 CI 假绿。
"""

from __future__ import annotations

from webdriver_client import (
    assert_reasoning_frame_sequence,
    clear_chat_stream_events,
    click_send,
    is_e2e_capture_installed,
    type_into_composer,
    wait_chat_stream_finish,
    wait_composer_ready,
)


# 长且需推理的提示词——能可靠触发上游模型走 thinking 路径。
# 短问答（如"你好"）几乎不会进入 reasoning，无法验证 lifecycle。
THINKING_PROMPT = (
    "请详细解释 Rust 的所有权（ownership）和借用（borrowing）机制，"
    "包括它们如何在编译期防止数据竞争。请举三个最小代码示例分别说明"
    "move 语义、可变借用排他性、以及生命周期标注的必要场景。"
)


def test_reasoning_frame_lifecycle_closes_with_non_empty_ids(session_id):
    assert is_e2e_capture_installed(session_id), (
        "dev hook __E2E_GET_EVENTS 未就绪——本场景要求 dev 构建启动："
        "`cargo tauri dev -f webdriver`（release bundle 不暴露 hook）"
    )

    s0 = wait_composer_ready(session_id, timeout=60.0)
    assert s0 is not None, "composer 未就绪"

    clear_chat_stream_events(session_id)
    type_into_composer(session_id, THINKING_PROMPT)
    click_send(session_id)

    events = wait_chat_stream_finish(session_id, timeout=180.0)
    assert events is not None, (
        "180s 内未收到 finish 帧——模型未配置 / 网络不通 / "
        "tauri_channel.respond() 未发 finish"
    )

    assert_reasoning_frame_sequence(events, require_reasoning=True)
