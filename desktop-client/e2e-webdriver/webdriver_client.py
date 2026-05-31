"""
最小 W3C WebDriver 客户端 —— 仅依赖标准库。

对接 `tauri-plugin-webdriver`（监听 127.0.0.1:4445）。
设计源自 e2e-webdriver-test-plan.md §0.2 的 Python 样板。
"""

from __future__ import annotations

import json
import socket
import time
import urllib.error
import urllib.request
from typing import Any, Callable, Optional

BASE_URL = "http://127.0.0.1:4445"


def is_driver_listening(host: str = "127.0.0.1", port: int = 4445, timeout: float = 0.5) -> bool:
    """快速探测 WebDriver 端口是否在监听。"""
    try:
        with socket.create_connection((host, port), timeout=timeout):
            return True
    except OSError:
        return False


def _req(method: str, path: str, body: Optional[dict] = None, timeout: float = 30.0) -> dict:
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(
        BASE_URL + path,
        data=data,
        method=method,
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return json.loads(resp.read().decode())


def new_session() -> str:
    """创建新的 WebDriver 会话，返回 sessionId。"""
    resp = _req("POST", "/session", {"capabilities": {"alwaysMatch": {}}})
    return resp["value"]["sessionId"]


def delete_session(session_id: str) -> None:
    """关闭会话，忽略 404 / 已关闭等错误。"""
    try:
        _req("DELETE", f"/session/{session_id}", timeout=5.0)
    except (urllib.error.URLError, urllib.error.HTTPError, OSError):
        pass


def execjs(session_id: str, script: str, args: Optional[list[Any]] = None) -> Any:
    """在被测 WKWebView 内同步执行 JS，返回脚本 return 值。

    脚本若 return Promise，sync 端点不会自动 await——请用 :func:`invoke`
    或 :func:`execjs_async` 处理异步调用。
    """
    resp = _req(
        "POST",
        f"/session/{session_id}/execute/sync",
        {"script": script, "args": args or []},
    )
    return resp["value"]


def execjs_async(
    session_id: str,
    script: str,
    args: Optional[list[Any]] = None,
    timeout: float = 30.0,
) -> Any:
    """在 WKWebView 内执行**异步**脚本（最后一个隐式参数是回调）。

    用于等待 Promise 的场景，例如 `window.__TAURI__.core.invoke(...)`。
    """
    resp = _req(
        "POST",
        f"/session/{session_id}/execute/async",
        {"script": script, "args": args or []},
        timeout=timeout,
    )
    return resp["value"]


def invoke(session_id: str, command: str, payload: Optional[dict] = None) -> Any:
    """调用 Tauri IPC 命令（自动 await Promise 并把异常转成可读字符串）。

    使用 `window.__TAURI_INTERNALS__.invoke`——Tauri v2 默认不暴露
    `window.__TAURI__`（除非 `app.withGlobalTauri = true`），但 internals
    通道始终存在。
    """
    script = (
        "const cmd = arguments[0];"
        "const payload = arguments[1];"
        "const cb = arguments[arguments.length - 1];"
        "try {"
        "  window.__TAURI_INTERNALS__.invoke(cmd, payload)"
        "    .then(v => cb({ ok: true, value: v }))"
        "    .catch(e => cb({ ok: false, error: String(e && e.message || e) }));"
        "} catch (e) {"
        "  cb({ ok: false, error: 'sync error: ' + String(e && e.message || e) });"
        "}"
    )
    # WebDriver 的 execute/async 把回调作为最后一个隐式参数追加到 arguments 上。
    result = execjs_async(session_id, script, [command, payload or {}])
    if not isinstance(result, dict) or not result.get("ok"):
        raise RuntimeError(
            f"invoke({command!r}) failed: {result.get('error') if isinstance(result, dict) else result}"
        )
    return result["value"]


def poll(fn: Callable[[], Any], timeout: float = 30.0, interval: float = 1.0) -> Any:
    """轮询直到 fn() 返回真值或超时；返回最后一次结果。

    fn() 抛 `URLError` / `HTTPError` / `OSError` 视为"页面还没准备好"，
    继续重试——典型场景是上一条用例刚 `location.reload()`，page agent 还在重启。
    """
    last = None
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            last = fn()
        except (urllib.error.URLError, urllib.error.HTTPError, OSError):
            last = None
        if last:
            return last
        time.sleep(interval)
    return last


# 复用的 DOM 探针：返回 JSON 字符串，由调用方 json.loads。
SNAPSHOT = r"""
const q = s => document.querySelectorAll(s).length;
const txt = s => { const e = document.querySelector(s); return e ? e.innerText : null; };
return JSON.stringify({
  boot:        !!document.querySelector('[data-testid="chat-runtime-boot-placeholder"]'),
  bootstrap:   !!document.querySelector('[data-testid="chat-runtime-bootstrap-placeholder"]'),
  historyLoad: !!document.querySelector('[data-testid="chat-runtime-history-loading"]'),
  composer:    q('[data-testid="composer-input"]') || q('textarea.aui-composer-input'),
  sendBtn:     q('[data-testid="composer-send"]') || q('button.aui-composer-send'),
  assistantMsgs: q('.aui-assistant-message-root, [data-role="assistant"]'),
  approvalCard: !!document.querySelector('[data-testid="approval-card"]'),
  approvalResult: txt('[data-testid="approval-result"]'),
  bodyHead:    document.body.innerText.replace(/\s+/g,' ').slice(0,120),
});
"""


def snapshot(session_id: str) -> dict:
    """便捷封装：跑 SNAPSHOT 探针并解析为 dict。"""
    return json.loads(execjs(session_id, SNAPSHOT))


# 优先 data-testid 选择器（plan §10-G1 修复后），回退 .aui-composer-input class 选择器。
_INJECT_COMPOSER = r"""
const [text] = arguments;
const ta = document.querySelector('[data-testid="composer-input"]')
         || document.querySelector('textarea.aui-composer-input');
if (!ta) return false;
const setter = Object.getOwnPropertyDescriptor(window.HTMLTextAreaElement.prototype, 'value').set;
setter.call(ta, text);
ta.dispatchEvent(new Event('input', { bubbles: true }));
return true;
"""


def type_into_composer(session_id: str, text: str) -> None:
    """把 text 注入 composer textarea 并触发 React 的 onChange。"""
    ok = execjs(session_id, _INJECT_COMPOSER, [text])
    if not ok:
        raise RuntimeError("找不到 composer-input（既无 data-testid 也无 .aui-composer-input）")


def click_send(session_id: str) -> None:
    """点击发送按钮：优先 [data-testid="composer-send"]，回退 .aui-composer-send。"""
    execjs(
        session_id,
        "(document.querySelector('[data-testid=\"composer-send\"]') "
        "|| document.querySelector('button.aui-composer-send')).click(); return true;",
    )


def wait_composer_ready(session_id: str, timeout: float = 60.0) -> Optional[dict]:
    """轮询直到 composer 出现且 boot 占位消失，返回当时的 snapshot。

    场景 2/3 的统一前置：发送消息前必须等聊天输入框就绪，否则 React 还没挂载
    完毕，注入文本会找不到元素。
    """

    def _check():
        s = snapshot(session_id)
        return s if (s["composer"] > 0 and not s["boot"]) else None

    return poll(_check, timeout=timeout, interval=1.0)


def wait_assistant_message_count(
    session_id: str,
    baseline: int,
    timeout: float = 120.0,
    interval: float = 2.0,
) -> Optional[dict]:
    """轮询直到 assistantMsgs 严格大于 baseline，返回当时的 snapshot。"""

    def _check():
        s = snapshot(session_id)
        return s if s["assistantMsgs"] > baseline else None

    return poll(_check, timeout=timeout, interval=interval)


# ---------------------------------------------------------------------------
# 审批卡片（场景 4）
# ---------------------------------------------------------------------------

def wait_approval_card(
    session_id: str,
    timeout: float = 120.0,
    interval: float = 1.5,
) -> Optional[dict]:
    """轮询直到 `[data-testid="approval-card"]` 出现，返回当时的 snapshot。

    超时返回 None。LLM 是否选择走需审批工具与模型/提示词有关——把这一判断
    留给上层断言（含可读的诊断 message）。
    """

    def _check():
        s = snapshot(session_id)
        return s if s["approvalCard"] else None

    return poll(_check, timeout=timeout, interval=interval)


def click_approval(session_id: str, decision: str) -> None:
    """点击审批卡片的 `批准` 或 `拒绝` 按钮。

    decision 必须是 "approve" 或 "deny"，对应 `[data-testid="approval-approve"]`
    / `[data-testid="approval-deny"]`（见 approval-tool-ui.tsx）。

    找不到按钮时抛 RuntimeError，避免静默失败。
    """
    if decision not in ("approve", "deny"):
        raise ValueError(f"decision 必须是 'approve' 或 'deny'，得到 {decision!r}")
    testid = f"approval-{decision}"
    script = (
        "const [tid] = arguments;"
        "const btn = document.querySelector(`[data-testid=\"${tid}\"]`);"
        "if (!btn) return false;"
        "btn.click();"
        "return true;"
    )
    ok = execjs(session_id, script, [testid])
    if not ok:
        raise RuntimeError(f"找不到审批按钮 [data-testid=\"{testid}\"]")


def read_approval_decision(session_id: str) -> Optional[bool]:
    """读 `[data-testid="approval-result"]` 上的 `data-approved` 属性。

    返回 True / False / None（横幅尚未出现或属性缺失）。
    """
    script = (
        "const e = document.querySelector('[data-testid=\"approval-result\"]');"
        "if (!e) return null;"
        "const v = e.getAttribute('data-approved');"
        "if (v === 'true') return true;"
        "if (v === 'false') return false;"
        "return null;"
    )
    return execjs(session_id, script)


def wait_approval_decision(
    session_id: str,
    expected_approved: bool,
    timeout: float = 30.0,
    interval: float = 1.0,
) -> bool:
    """轮询直到 `data-approved` 等于 expected_approved，返回是否命中。"""

    def _check():
        v = read_approval_decision(session_id)
        return True if v is expected_approved else None

    return bool(poll(_check, timeout=timeout, interval=interval))


def clear_pending_approval(session_id: str, timeout: float = 5.0) -> bool:
    """若存在挂起的审批卡片，点 deny 把它清掉。

    用例之间共享同一 WKWebView，前一条用例可能留下未点击的卡片；本函数让
    每条用例从"无挂起审批"的干净状态开始。返回是否实际清理过。
    """
    s = snapshot(session_id)
    if not s["approvalCard"]:
        return False
    click_approval(session_id, "deny")
    # 等横幅出现，确保 SDK 已把状态推进，避免下一条用例还看到旧卡片
    wait_approval_decision(session_id, expected_approved=False, timeout=timeout)
    return True

