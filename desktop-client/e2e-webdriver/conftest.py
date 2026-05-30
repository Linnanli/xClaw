"""
pytest 共享夹具：自动跳过未启动场景 + 管理 WebDriver 会话生命周期。
"""

from __future__ import annotations

import pytest

from webdriver_client import delete_session, is_driver_listening, new_session


def pytest_collection_modifyitems(config, items):
    """4445 未监听时整体 skip，避免本地/CI 噪声。"""
    if is_driver_listening():
        return
    skip_marker = pytest.mark.skip(
        reason="tauri-plugin-webdriver 未监听 127.0.0.1:4445；"
        "请先在另一终端跑 `cargo tauri dev -f webdriver`（见 README）。"
    )
    for item in items:
        item.add_marker(skip_marker)


@pytest.fixture
def session_id():
    """提供一次性 WebDriver 会话，结束自动 delete。"""
    sid = new_session()
    try:
        yield sid
    finally:
        delete_session(sid)
