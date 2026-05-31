# E2E (tauri-plugin-webdriver / WKWebView)

> 真理来源：[../docs/e2e-webdriver-test-plan.md](../docs/e2e-webdriver-test-plan.md)

本目录用 `tauri-plugin-webdriver` 直接驱动真实 Tauri 窗口的 WKWebView，监听
`http://127.0.0.1:4445`，通过 `window.__TAURI__.core.invoke` 调真实 IPC。

与同级 `e2e/`（wdio + tauri-driver + Lima VM，跑在 Linux）是**两条独立链路**，
本目录跑在 **macOS 宿主机**上，覆盖 WKWebView 真链路。

## 前置

- macOS
- Python ≥ 3.10（仅用 stdlib `urllib`/`json`/`time` + `pytest`）
- 已 `pip install pytest`

## 启动被测应用（另开终端，保持运行）

```bash
cd desktop-client
set -a && source ./.env && set +a
export MANAGED_MODE=false
export OPENAI_API_KEY="$LLM_API_KEY"
cargo tauri dev -f webdriver
```

等到终端出现 WebView 起来 + `lsof -ti:4445` 有进程后再跑测试。

## 跑测试

```bash
# 全部
pytest desktop-client/e2e-webdriver -v

# 只跑场景 1
pytest desktop-client/e2e-webdriver/tests/test_scenario_01_engine_readiness.py -v
```

若 4445 未监听，所有用例会被 **skip**（不会假阳红 CI / 本地噪声）。

## 当前覆盖范围

| 场景 | 状态 |
|------|------|
| §1 引擎就绪与刷新韧性 | ✅ 已落地 |
| §2 Agent 基础执行 | ⬜ 待落地 |
| §3–§7 | ⬜ 待落地（按 plan §1–§7 顺序推进） |
