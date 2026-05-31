"""
场景 2 — Agent 基础执行

对应：desktop-client/docs/e2e-webdriver-test-plan.md §2

验证：在 composer 输入一句话 + 点击发送后，60–120s 内至少出现 1 个新的助手气泡。

前置条件：
  - 场景 1 已就绪（compose 出现、boot 占位消失）
  - 应用端已通过 admin 注入并激活模型（本地开发环境默认带 glm-4.7-flash）

LLM 不通时，这条用例预期失败而非 skip——失败本身就是有用的信号，
说明 send_chat_message 链路或模型配置出了问题。
"""

from __future__ import annotations

from webdriver_client import (
    click_send,
    poll,
    snapshot,
    type_into_composer,
)


def _wait_composer_ready(sid: str, timeout: float = 60.0) -> dict | None:
    def _check():
        s = snapshot(sid)
        return s if (s["composer"] > 0 and not s["boot"]) else None

    return poll(_check, timeout=timeout, interval=1.0)


def test_send_message_yields_assistant_reply(session_id):
    """输入 → 发送 → 出现新助手气泡。

    取发送前的 assistantMsgs 计数作为 baseline，断言之后的计数 > baseline。
    """
    s0 = _wait_composer_ready(session_id, timeout=60.0)
    assert s0 is not None, "场景 1 前置：composer 未就绪"
    baseline = s0["assistantMsgs"]

    type_into_composer(session_id, "用一句话介绍你自己")
    click_send(session_id)

    def _has_new_reply():
        s = snapshot(session_id)
        return s if s["assistantMsgs"] > baseline else None

    s1 = poll(_has_new_reply, timeout=120.0, interval=2.0)
    assert s1 is not None, (
        "120s 内未出现新助手气泡——可能 LLM 未配置 / send_chat_message 失败 / 网络中断"
    )
    assert s1["assistantMsgs"] > baseline, (
        f"assistantMsgs 未增长（baseline={baseline}, now={s1['assistantMsgs']}）"
    )
