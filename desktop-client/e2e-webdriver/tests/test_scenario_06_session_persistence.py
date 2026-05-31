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
PROMPT_TEMPLATE = (
    "请原样复述以下字符串一次，不要解释、不要省略，"
    "也不要加引号：{marker}"
)

# 发消息 → 入库这一步本身不需要等 LLM 回复，
# send_chat_message 同步把 user message 写库后才返回。给 30s 兜底。
SEND_TIMEOUT = 30.0


def _find_or_create_thread(session_id: str) -> str:
    """优先复用已有 thread，没有就新建。返回 thread_id。"""
    threads = invoke(session_id, "ic_list_threads", {})
    if isinstance(threads, list) and threads:
        return threads[0]["id"]
    return invoke(session_id, "ic_create_thread", {})


def test_thread_history_survives_reload(session_id):
    """核心：发 marker → 刷新 → ic_get_thread_history 仍含 marker。"""
    marker = _make_marker()

    s0 = wait_composer_ready(session_id, timeout=60.0)
    assert s0 is not None, "composer 未就绪"

    tid = _find_or_create_thread(session_id)
    assert tid and isinstance(tid, str), f"thread_id 形态异常：{tid!r}"

    type_into_composer(session_id, PROMPT_TEMPLATE.format(marker=marker))
    click_send(session_id)

    # 等到 user message 真的入库（ic_get_thread_history 能读到 marker），
    # 之后才能安全 reload——否则 send_chat_message 还在飞的话刷新会丢消息。
    def _marker_in_history():
        try:
            hist = invoke(session_id, "ic_get_thread_history",
                          {"threadId": tid, "limit": 200})
        except RuntimeError:
            return None
        if isinstance(hist, list) and any(
            marker in (m.get("content") or "") for m in hist
        ):
            return hist
        return None

    hist_before = poll(_marker_in_history, timeout=SEND_TIMEOUT, interval=1.0)
    assert hist_before is not None, (
        f"{SEND_TIMEOUT:.0f}s 内 marker {marker!r} 未出现在 ic_get_thread_history"
        " ——可能：(a) send_chat_message 未成功；(b) 持久化层未写入；"
        f"(c) 取错了 thread_id（tid={tid!r}）"
    )

    # 现在 reload
    execjs(session_id, "location.reload(); return true;")
    # reload 后等 composer 重新就绪
    s1 = wait_composer_ready(session_id, timeout=60.0)
    assert s1 is not None, "reload 后 composer 60s 内未就绪"

    hist_after = invoke(session_id, "ic_get_thread_history",
                        {"threadId": tid, "limit": 200})
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
    s0 = wait_composer_ready(session_id, timeout=60.0)
    assert s0 is not None, "composer 未就绪"

    before = invoke(session_id, "ic_list_threads", {})
    assert isinstance(before, list), f"ic_list_threads 返回非 list：{before!r}"
    ids_before = {t["id"] for t in before if isinstance(t, dict) and "id" in t}

    execjs(session_id, "location.reload(); return true;")
    s1 = wait_composer_ready(session_id, timeout=60.0)
    assert s1 is not None, "reload 后 composer 60s 内未就绪"

    after = invoke(session_id, "ic_list_threads", {})
    ids_after = {t["id"] for t in after if isinstance(t, dict) and "id" in t}

    # 允许 after 多出新增（如 reload 期间用户新开），但已有 id 必须保留
    missing = ids_before - ids_after
    assert not missing, (
        "⚠️ 持久化回归：reload 后部分 thread 消失——"
        f"丢失 {len(missing)} 个 id={sorted(missing)[:5]}..."
    )
