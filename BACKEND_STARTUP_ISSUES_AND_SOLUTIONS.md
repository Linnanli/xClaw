# 后端启动问题总结和解决方案

## 问题 1: 后端进入 REPL 模式导致 HTTP 无响应

### 问题描述
- 后端启动后进入交互式 REPL（Read-Eval-Print Loop）
- REPL 等待用户输入，阻止 HTTP 服务器响应请求
- 导致所有 HTTP 请求超时

### 原因
- 使用 `cargo run -- run --cli-only --no-onboard` 启动
- `--cli-only` 启用了交互式 REPL 模式
- 后端进程的 stdin 连接到终端，导致 REPL 等待输入

### 解决方案
```bash
# ❌ 错误做法
cargo run -- run --cli-only --no-onboard

# ✅ 正确做法 1: 不使用 --cli-only
cargo run -- run --no-onboard

# ✅ 正确做法 2: 重定向 stdin（推荐）
cargo run -- run --no-onboard < /dev/null

# ✅ 正确做法 3: 后台运行并重定向
nohup cargo run -- run --no-onboard < /dev/null > /tmp/backend.log 2>&1 &
```

---

## 问题 2: 旧进程仍在运行导致启动失败

### 问题描述
```
Error: Another IronClaw instance is already running (PID 5567). 
If this is incorrect, remove the stale PID file: /Users/nallylin/.ironclaw/ironclaw.pid
```

### 原因
- 前一个后端进程没有正确关闭
- PID 文件仍然存在
- 新启动的进程检测到旧进程仍在运行

### 解决方案
```bash
# 1. 杀死旧进程
kill -9 <PID>

# 2. 删除 PID 文件
rm -f ~/.ironclaw/ironclaw.pid

# 3. 清理所有 cargo 进程
pkill -f "cargo run"

# 4. 清理端口
lsof -i :3000 | grep -v COMMAND | awk '{print $2}' | xargs kill -9
```

---

## 问题 3: 认证令牌过期

### 问题描述
```
❌ 创建线程返回错误 401 Unauthorized: Invalid or missing auth token
```

### 原因
- 每次启动后端时都会生成新的认证令牌
- 前端和测试使用的是旧令牌
- 令牌不匹配导致认证失败

### 解决方案
```bash
# 1. 启动后端后获取新令牌
grep "gateway" /tmp/backend.log | tail -1

# 2. 更新前端令牌
# 文件: desktop-client/src-ui/src/app/components/tabs/ChatTabWithAiSdk.tsx
authToken: 'd397b61ad5584603d5691f03e73a0a3eea6fd66d1590ddc64a6b0292c7a2f270'

# 3. 更新测试令牌
# 文件: desktop-client/tests/qwen_e2e_test.rs
const AUTH_TOKEN: &str = "d397b61ad5584603d5691f03e73a0a3eea6fd66d1590ddc64a6b0292c7a2f270";
```

---

## 问题 4: LLM_API_KEY 未设置

### 问题描述
- 后端启动时没有设置 Qwen API 密钥
- 后端无法调用 Qwen API

### 原因
- 环境变量未导出
- 启动脚本没有检查环境变量

### 解决方案
```bash
# 1. 设置环境变量
export LLM_API_KEY="sk-d162b1775e514757b083a2dfe729d6e6"
export LLM_BACKEND="openai_compatible"
export LLM_BASE_URL="https://dashscope.aliyuncs.com/compatible-mode/v1"
export LLM_MODEL="qwen-max"

# 2. 验证环境变量
echo $LLM_API_KEY

# 3. 启动后端
cargo run -- run --no-onboard < /dev/null
```

---

## 完整的启动流程

### 清理旧进程
```bash
pkill -f "cargo run" || true
sleep 2
rm -f ~/.ironclaw/ironclaw.pid
lsof -i :3000 | grep -v COMMAND | awk '{print $2}' | xargs kill -9 2>/dev/null || true
sleep 2
```

### 设置环境变量
```bash
export LLM_API_KEY="sk-d162b1775e514757b083a2dfe729d6e6"
export LLM_BACKEND="openai_compatible"
export LLM_BASE_URL="https://dashscope.aliyuncs.com/compatible-mode/v1"
export LLM_MODEL="qwen-max"
```

### 启动后端
```bash
nohup cargo run -- run --no-onboard < /dev/null > /tmp/backend.log 2>&1 &
sleep 15
```

### 验证后端
```bash
curl -s http://localhost:3000/api/health
grep "gateway" /tmp/backend.log | tail -1
```

### 启动前端
```bash
cd desktop-client/src-ui
npm run dev
```

---

## 关键要点

| 问题 | 原因 | 解决方案 |
|------|------|--------|
| HTTP 超时 | REPL 阻塞 | 使用 `< /dev/null` 重定向 |
| 启动失败 | 旧进程仍运行 | 杀死旧进程并删除 PID 文件 |
| 认证失败 | 令牌过期 | 每次启动后更新令牌 |
| API 调用失败 | 环境变量未设置 | 导出所有必需的环境变量 |

---

## 最佳实践

1. **总是清理旧进程** - 启动前清理所有旧进程
2. **使用 stdin 重定向** - 禁用 REPL 交互模式
3. **检查环境变量** - 确保所有必需的环境变量都已设置
4. **验证启动** - 启动后立即验证后端健康状态
5. **保存日志** - 使用 `nohup` 和日志重定向便于调试

