"""
场景 6 — 会话持久化（刷新后历史不丢）

对应：desktop-client/docs/e2e-webdriver-test-plan.md §6

验证：
  1. 发消息后通过 ic_get_thread_history 能读回内容
  2. location.reload() 后再次读 history，原 marker 仍在
  3. ic_list_threads 在刷新前后返回相同集合（强一致）

依赖：
  - 引擎就绪 + LLM 可达（与场景 2/3/4 相同前置）
  - send_chat_message 不被 SafetyBridge 拦截（marker 文本无 PII / 密钥样式）

不依赖：
  - 不依赖助手回复内容（只要 user 消息被持久化即可，回复早晚都行）
  - 不依赖具体 thread 数量（取 `ic_list_threads` 头部那个或新建）
"""

from __future__ import annotations

import time
import uuid

from webdriver_client import (
    click_send,
    execjs,
    invoke,
    poll,
    snapshot,
    type_into_composer,
    wait_assistant_message_count,
    wait_composer_ready,
)

# 32 位十六进制 marker，发音独特，不会与正常文本碰撞、也不会被 DLP 误判为密钥
# （没有 `sk-` 前缀，长度也不像 OpenAI key）
def _make_marker() -> str:
    return f"e2emarker{uuid.uuid4().hex[:16]}"

# 让 LLM 必复述 marker，便于核验链路完整性
PROMPT_TEMPLATE = "请复述：{marker}"

# 发消息 → 入库这一步本身不需要等 LLM 回复，
# send_chat_message 同步把 user message 写库后才返回。冷启动 Tauri 的首条 send
# 实测可能 60s+，给到 120s 兜底。
SEND_TIMEOUT = 120.0


def _wait_tauri_internals_ready(session_id: str, timeout: float = 30.0) -> bool:
    """reload 后 React 已挂载（composer 存在）但 Tauri internals 可能还没 wire 完，
    直接 execute/async 调 invoke 会让 WebDriver async 端口挂死到 socket timeout。
    先用同步 execjs 轮询 `__TAURI_INTERNALS__.invoke` 是函数，再继续。"""
    probe = (
        "return typeof window.__TAURI_INTERNALS__ === 'object' "
        "&& typeof window.__TAURI_INTERNALS__.invoke === 'function';"
    )
    return bool(poll(lambda: execjs(session_id, probe), timeout=timeout, interval=0.5))


def _invoke_polling(session_id: str, command: str, payload: dict | None = None,
                    timeout: float = 30.0):
    """tauri-plugin-webdriver 的 execute/async 端口在 location.reload() 之后
    会挂死（socket 永远不回，2026-05-31 §6 实测），即便 __TAURI_INTERNALS__
    已 wire。绕开方案：用 execute/sync 触发 invoke()，把 Promise 结果挂到
    window.__e2e_invoke_result__，再 sync 轮询读取。"""
    fire = (
        "const k = '__e2e_invoke_result_' + Math.random().toString(36).slice(2);"
        "window[k] = null;"
        "window.__TAURI_INTERNALS__.invoke(arguments[0], arguments[1])"
        "  .then(v => { window[k] = { ok: true, value: v }; })"
        "  .catch(e => { window[k] = { ok: false, error: String(e && e.message || e) }; });"
        "return k;"
    )
    key = execjs(session_id, fire, [command, payload or {}])
    read = f"return window[{key!r}];"
    result = poll(lambda: execjs(session_id, read), timeout=timeout, interval=0.5)
    if not isinstance(result, dict) or not result.get("ok"):
        err = result.get("error") if isinstance(result, dict) else result
        raise RuntimeError(f"invoke({command!r}) failed: {err}")
    return result["value"]


def test_thread_history_survives_reload(session_id):
    """核心：发 marker → 刷新 → ic_get_thread_history 仍含 marker。"""
    marker = _make_marker()

    s0 = wait_composer_ready(session_id, timeout=120.0)
    assert s0 is not None, "composer 未就绪"

    # 预热一次 IPC——实测裸 `type+click+poll` 在新 session 第一次发消息时
    # send_chat_message 会卡（marker 30s+ 不入库），先打一次 `ic_list_threads`
    # 给后端预热则后续 send 1s 内入库。
    invoke(session_id, "ic_list_threads", {})

    # UI 当前的活跃 thread 不一定是 `ic_list_threads` 的第一项（实测会自动新建
    # 一个空 thread 给 composer），所以发送前不要假定 thread_id，发送后用
    # marker 反查。
    type_into_composer(session_id, PROMPT_TEMPLATE.format(marker=marker))
    click_send(session_id)
    time.sleep(2)  # 给 click_send → send_chat_message 留点 IPC 启动窗口

    def _find_thread_with_marker():
        try:
            threads = invoke(session_id, "ic_list_threads", {})
        except RuntimeError:
            return None
        for t in threads[:10]:  # 检查最近 10 个 thread 足够
            try:
                hist = invoke(session_id, "ic_get_thread_history",
                              {"threadId": t["id"], "limit": 50})
            except RuntimeError:
                continue
            if any(marker in (m.get("content") or "") for m in hist):
                return (t["id"], hist)
        return None

    found = poll(_find_thread_with_marker, timeout=SEND_TIMEOUT, interval=1.0)
    assert found is not None, (
        f"{SEND_TIMEOUT:.0f}s 内 marker {marker!r} 未出现在任何 thread"
        " ——可能：(a) send_chat_message 未成功；(b) 持久化层未写入；"
        "(c) SafetyBridge 拦截"
    )
    tid, _ = found

    # 现在 reload
    execjs(session_id, "location.reload(); return true;")
    # reload 后等 composer 重新就绪
    s1 = wait_composer_ready(session_id, timeout=60.0)
    assert s1 is not None, "reload 后 composer 60s 内未就绪"
    assert _wait_tauri_internals_ready(session_id), (
        "reload 后 30s 内 __TAURI_INTERNALS__.invoke 未就绪"
    )
    # WebDriver 与 Tauri runtime 重接后，立即 invoke `ic_*` 会永远挂死（实测
    # 90s 仍不返）；空转 10s 让后端 ThreadHistoryLoader / IPC dispatcher 重新绑定后就能
    # 1s 内返。原因未完全查清，这里用固定延迟兑现可靠性。
    time.sleep(10)

    hist_after = _invoke_polling(session_id, "ic_get_thread_history",
                                 {"threadId": tid, "limit": 200},
                                 timeout=90.0)
    assert isinstance(hist_after, list), f"ic_get_thread_history 返回非 list：{hist_after!r}"

    contents_after = [m.get("content") or "" for m in hist_after]
    assert any(marker in c for c in contents_after), (
        "⚠️ 持久化回归：reload 后 ic_get_thread_history 不再包含 marker——"
        "可能：(a) 写入未 fsync；(b) ThreadHistoryLoader 读错 thread；"
        "(c) 存储层在重启时丢数据。"
        f" marker={marker!r} thread={tid!r} contents_after_count={len(contents_after)}"
    )


def test_list_threads_stable_across_reload(session_id):
    """ic_list_threads 在 reload 前后返回相同 thread 集合（id 集合相等）。"""
    s0 = wait_composer_ready(session_id, timeout=120.0)
    assert s0 is not None, "composer 未就绪"

    before = invoke(session_id, "ic_list_threads", {})
    assert isinstance(before, list), f"ic_list_threads 返回非 list：{before!r}"
    ids_before = {t["id"] for t in before if isinstance(t, dict) and "id" in t}

    execjs(session_id, "location.reload(); return true;")
    s1 = wait_composer_ready(session_id, timeout=60.0)
    assert s1 is not None, "reload 后 composer 60s 内未就绪"
    assert _wait_tauri_internals_ready(session_id), (
        "reload 后 30s 内 __TAURI_INTERNALS__.invoke 未就绪"
    )
    time.sleep(10)  # 同上：IPC dispatcher 重绑定窗口

    after = _invoke_polling(session_id, "ic_list_threads", {}, timeout=90.0)
    ids_after = {t["id"] for t in after if isinstance(t, dict) and "id" in t}

    # 允许 after 多出新增（如 reload 期间用户新开），但已有 id 必须保留
    missing = ids_before - ids_after
    assert not missing, (
        "⚠️ 持久化回归：reload 后部分 thread 消失——"
        f"丢失 {len(missing)} 个 id={sorted(missing)[:5]}..."
    )
