"""
场景 4 — 工具分发与审批（Fail-Safe）

对应：desktop-client/docs/e2e-webdriver-test-plan.md §4

验证：
  1. 触发需审批的写操作 → 必须弹出审批卡片（**不能**静默执行，Fail-Safe）
  2. 点击「拒绝」→ 结果横幅 `data-approved=false`
    3. 点击「批准」→ 结果横幅 `data-approved=true`，且探针文件被测试清理

为什么不断言 `AGENT_AUTO_APPROVE_TOOLS=false` 环境变量（plan §10-G3）：
  该变量在全仓 `crates/` + `desktop-client/` 下零匹配，是文档残留的伪不变量。
  正确的 Fail-Safe 判据是 **行为级**：写操作触发审批卡片，而不是检查某个不存在的开关。

approve 分支的污染控制：
    审批通过后会真实写文件，因此用例只允许写入 e2e-webdriver/.tmp/ 下的探针文件，
    并在测试前后清理该路径与旧版 e2e_probe.txt 探针，避免跨用例 / 跨运行污染。

测试之间用 `clear_pending_approval` 在开头清掉上一条用例可能留下的挂起卡片，
避免共享同一 WKWebView 时的串扰。
"""

from __future__ import annotations

from contextlib import suppress
from pathlib import Path

import pytest

from webdriver_client import (
    clear_pending_approval,
    click_approval,
    click_send,
    snapshot,
    type_into_composer,
    wait_approval_card,
    wait_approval_decision,
    wait_composer_ready,
)

DESKTOP_CLIENT_DIR = Path(__file__).resolve().parents[2]
PROBE_RELATIVE_PATH = "e2e-webdriver/.tmp/e2e_probe_approval.txt"
PROBE_PATH = DESKTOP_CLIENT_DIR / PROBE_RELATIVE_PATH
LEGACY_PROBE_PATH = DESKTOP_CLIENT_DIR / "e2e_probe.txt"

# 提示词照搬 plan §4，最大概率让 LLM 选择走写文件类需审批工具。
WRITE_PROMPT = (
    f"请立即调用 write_file 工具在当前工作目录创建文件 {PROBE_RELATIVE_PATH}，"
    "内容为字符串 hello。不要先解释，不要先用 read_file/list_dir 探查，"
    "也不要建议我用其它方式——直接调用 write_file。"
)

# LLM 选择 + 模型推理 + 后端审批通知双发 + 前端 SDK 路由到 ToolUI 整条链路，
# 实测在 macOS + glm-4.7-flash 一般 < 30s，给 120s 兜底应对偶发慢响应。
CARD_TIMEOUT = 120.0


def cleanup_probe_files() -> None:
    """清理本场景可能产生的探针文件。"""
    PROBE_PATH.unlink(missing_ok=True)
    LEGACY_PROBE_PATH.unlink(missing_ok=True)
    with suppress(OSError):
        PROBE_PATH.parent.rmdir()


def prepare_probe_dir() -> None:
    cleanup_probe_files()
    PROBE_PATH.parent.mkdir(parents=True, exist_ok=True)


@pytest.fixture
def probe_workspace():
    prepare_probe_dir()
    try:
        yield
    finally:
        cleanup_probe_files()


def test_write_tool_triggers_approval_card(session_id, probe_workspace):
    """Fail-Safe：发"新建文件"指令 → 审批卡片出现，且不点按钮。

    这是 plan §4 的核心 Fail-Safe 不变量——写操作**绝不能**绕过审批静默执行。
    断言只要求卡片出现（`approvalCard` 为真），不点 approve/deny；点 deny 仅作为
    tearDown 清理状态。
    """
    clear_pending_approval(session_id)

    s0 = wait_composer_ready(session_id, timeout=60.0)
    assert s0 is not None, "composer 未就绪"

    type_into_composer(session_id, WRITE_PROMPT)
    click_send(session_id)

    s1 = wait_approval_card(session_id, timeout=CARD_TIMEOUT)
    assert s1 is not None, (
        f"{CARD_TIMEOUT:.0f}s 内未出现审批卡片——可能：(a) LLM 未配置/不可达；"
        "(b) 模型没选择写文件工具（提示词不够直接）；"
        "(c) ⚠️ Fail-Open：写操作被静默执行——这是严重安全回归。"
        f" 末态 snapshot={snapshot(session_id)}"
    )
    assert s1["approvalCard"] is True, "snapshot 自相矛盾"
    assert not PROBE_PATH.exists(), "审批前探针文件已被写入，疑似 Fail-Open 回归"

    # 清掉挂起审批，避免污染后续用例
    clear_pending_approval(session_id)


def test_deny_yields_approved_false(session_id, probe_workspace):
    """完整链路：发指令 → 等卡片 → 点拒绝 → 等 `data-approved=false` 横幅。

    覆盖前端 `ic_deny_tool` IPC → 后端 `Submission::ExecApproval` → SDK 把工具调用
    推进到 `complete` → ApprovalResultBanner 渲染 `data-approved=false` 这条回路。
    """
    clear_pending_approval(session_id)

    s0 = wait_composer_ready(session_id, timeout=60.0)
    assert s0 is not None, "composer 未就绪"

    type_into_composer(session_id, WRITE_PROMPT)
    click_send(session_id)

    s1 = wait_approval_card(session_id, timeout=CARD_TIMEOUT)
    assert s1 is not None, (
        f"{CARD_TIMEOUT:.0f}s 内未出现审批卡片，无法继续验证拒绝路径。"
        f" 末态 snapshot={snapshot(session_id)}"
    )

    click_approval(session_id, "deny")

    denied = wait_approval_decision(
        session_id, expected_approved=False, timeout=30.0
    )
    assert denied, (
        "点 deny 后 30s 内未出现 `data-approved=false` 横幅——可能："
        "(a) ic_deny_tool IPC 未送达；(b) 后端未把虚拟工具推进到 complete；"
        f"(c) ApprovalResultBanner 渲染失败。 末态 snapshot={snapshot(session_id)}"
    )
    assert not PROBE_PATH.exists(), "拒绝审批后探针文件仍被写入，疑似审批 gate 失效"


def test_approve_yields_approved_true(session_id, probe_workspace):
    """批准路径：发指令 → 等卡片 → 点批准 → 横幅 true → 探针文件落地后清理。"""
    clear_pending_approval(session_id)

    s0 = wait_composer_ready(session_id, timeout=60.0)
    assert s0 is not None, "composer 未就绪"

    type_into_composer(session_id, WRITE_PROMPT)
    click_send(session_id)

    s1 = wait_approval_card(session_id, timeout=CARD_TIMEOUT)
    assert s1 is not None, (
        f"{CARD_TIMEOUT:.0f}s 内未出现审批卡片，无法继续验证批准路径。"
        f" 末态 snapshot={snapshot(session_id)}"
    )

    click_approval(session_id, "approve")
    approved = wait_approval_decision(
        session_id, expected_approved=True, timeout=30.0
    )
    assert approved, (
        "点 approve 后 30s 内未出现 `data-approved=true` 横幅——可能："
        "(a) ic_approve_tool IPC 未送达；(b) 后端未执行批准后的工具调用；"
        f"(c) ApprovalResultBanner 渲染失败。 末态 snapshot={snapshot(session_id)}"
    )
    assert PROBE_PATH.exists(), f"批准后探针文件未落地：{PROBE_PATH}"
    assert PROBE_PATH.read_text(encoding="utf-8").strip() == "hello"
