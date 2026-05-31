"""
场景 1 — 引擎就绪与刷新韧性

对应：desktop-client/docs/e2e-webdriver-test-plan.md §1

验证：
  (a) 冷启动从 "引擎启动中…" 占位过渡到就绪（composer 出现）
  (b) `get_engine_status` 命令返回 ready=true / failed=false
  (c) webview reload 后仍能就绪——核心是 `useEngineReady` 挂载回查，
      不依赖一次性广播
"""

from __future__ import annotations

from webdriver_client import execjs, invoke, poll, snapshot


def _wait_ready(sid: str, timeout: float = 60.0) -> dict | None:
    """轮询直到 composer 出现且 boot 占位消失。"""

    def _check():
        s = snapshot(sid)
        return s if (s["composer"] > 0 and not s["boot"]) else None

    return poll(_check, timeout=timeout, interval=1.0)


def test_cold_boot_reaches_ready(session_id):
    """(a) 冷启动应在 60s 内进入就绪态：boot 占位消失、composer 出现。"""
    s = _wait_ready(session_id, timeout=60.0)
    assert s is not None, "冷启动未就绪（poll 超时）"
    assert s["composer"] > 0, f"无聊天输入框；snapshot={s}"
    assert not s["boot"], f"boot 占位未消失；snapshot={s}"


def test_get_engine_status_reports_ready(session_id):
    """(b) `get_engine_status` 命令应直接返回 ready=true / failed=false。"""
    _wait_ready(session_id, timeout=60.0)

    status = invoke(session_id, "get_engine_status")
    assert status is not None, "get_engine_status 返回 null"
    assert status.get("ready") is True, f"引擎未就绪：{status}"
    assert status.get("failed") is False, f"引擎被标记为 failed：{status}"


def test_reload_remains_ready(session_id):
    """(c) webview reload 后必须重新就绪——挂载回查不能失效。"""
    _wait_ready(session_id, timeout=60.0)

    execjs(session_id, "location.reload();")

    s2 = _wait_ready(session_id, timeout=30.0)
    assert s2 is not None, "reload 后 30s 内仍未就绪（回查失效 / 永久卡 boot 占位）"
    assert s2["composer"] > 0, f"reload 后无聊天输入框；snapshot={s2}"
    assert not s2["boot"], f"reload 后 boot 占位未消失；snapshot={s2}"
