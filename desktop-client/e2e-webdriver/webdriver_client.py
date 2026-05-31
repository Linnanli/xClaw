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
    """轮询直到 fn() 返回真值或超时；返回最后一次结果。"""
    last = None
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        last = fn()
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
  composer:    q('textarea.aui-composer-input'),
  sendBtn:     q('button.aui-composer-send'),
  assistantMsgs: q('.aui-assistant-message-root, [data-role="assistant"]'),
  approvalCard: !!document.querySelector('[data-testid="approval-card"]'),
  approvalResult: txt('[data-testid="approval-result"]'),
  bodyHead:    document.body.innerText.replace(/\s+/g,' ').slice(0,120),
});
"""


def snapshot(session_id: str) -> dict:
    """便捷封装：跑 SNAPSHOT 探针并解析为 dict。"""
    return json.loads(execjs(session_id, SNAPSHOT))
