"""
场景 3 — 工具注册与发现

对应：desktop-client/docs/e2e-webdriver-test-plan.md §3

验证：引擎启动时注册的**只读工具**（如 `list_dir`）能被对话触发并产出助手回复，
即整条「Prompt → LLM → ToolCall → Dispatcher → ToolResult → 助手气泡」分发链路通畅。

为什么不断言 `tools: N accepted` 日志（plan §10-G2）：
  `summary_line()` / `log_registration_report()` 只在 `bootstrap_tools()` 内触发，
  桌面端 `engine.rs` 走的是手动 `register_message_tools` + `register_job_tools`，
  那行 info 日志在桌面端不一定出现，断言它会假阴。改为「发对话 + 等助手回复」
  的功能验证路线。

LLM 不可达 / 模型未配置时，这条用例预期失败而非 skip——失败本身是有效信号。
"""

from __future__ import annotations

from webdriver_client import (
    click_send,
    snapshot,
    type_into_composer,
    wait_assistant_message_count,
    wait_composer_ready,
)


def test_only_readonly_tool_dispatch_yields_reply(session_id):
    """触发只读工具 `list_dir` → 90s 内出现新助手气泡。

    Plan §3 给的提示词照搬：「列出当前工作目录下的文件，用 list_dir 工具」。
    断言点取「assistant 气泡数严格增长」，不去解析具体文本——LLM 输出文本不
    稳定（中英文/换行/转义），但只要分发成功就一定有新气泡。
    """
    s0 = wait_composer_ready(session_id, timeout=60.0)
    assert s0 is not None, "场景 1 前置：composer 未就绪"
    baseline = s0["assistantMsgs"]

    type_into_composer(session_id, "列出当前工作目录下的文件，用 list_dir 工具")
    click_send(session_id)

    s1 = wait_assistant_message_count(
        session_id, baseline=baseline, timeout=90.0, interval=2.0
    )

    assert s1 is not None, (
        "90s 内未出现新助手气泡——可能：(a) LLM 未配置或不可达；"
        "(b) 模型没选择调用 list_dir；(c) Dispatcher 链路断（注册缺失 / 调度失败）。"
        f" 末态 snapshot={snapshot(session_id)}"
    )
    assert s1["assistantMsgs"] > baseline, (
        f"assistantMsgs 未严格增长（baseline={baseline}, now={s1['assistantMsgs']}）"
    )
