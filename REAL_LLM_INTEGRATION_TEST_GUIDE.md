# 使用真实 LLM API 的集成测试指南

## 概述

本指南说明如何运行使用真实 LLM API（而不是 mock 数据）的集成测试，验证通过聊天创建任务的完整流程。

## 测试文件

### 1. `tests/e2e_real_llm_job_creation.rs`

基础集成测试，验证任务创建和管理功能。

**测试用例**：
- `test_create_job_via_chat_real_llm` - 通过聊天创建任务
- `test_create_multiple_jobs_real_llm` - 创建多个任务
- `test_job_state_transitions_real_llm` - 验证任务状态转换

### 2. `tests/e2e_real_llm_chat_job_creation.rs`

高级集成测试，验证完整的聊天工作流程。

**测试用例**：
- `test_chat_create_job_with_real_llm` - 通过聊天创建任务
- `test_chat_job_workflow_with_real_llm` - 完整的任务工作流程
- `test_llm_job_understanding_with_real_llm` - LLM 对任务请求的理解

## 前置条件

### 1. 启动后端服务

```bash
# 使用 Qwen API
export LLM_API_KEY="sk-d162b1775e514757b083a2dfe729d6e6"
export LLM_BACKEND="openai_compatible"
export LLM_BASE_URL="https://dashscope.aliyuncs.com/compatible-mode/v1"
export LLM_MODEL="qwen-max"

# 启动后端
cargo run -- run --no-onboard < /dev/null &
```

### 2. 配置 LLM API

#### 选项 A：Qwen（推荐）

```bash
export LLM_API_KEY="sk-d162b1775e514757b083a2dfe729d6e6"
export LLM_BACKEND="openai_compatible"
export LLM_BASE_URL="https://dashscope.aliyuncs.com/compatible-mode/v1"
export LLM_MODEL="qwen-max"
```

#### 选项 B：Claude（Anthropic）

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
export LLM_BACKEND="anthropic"
export ANTHROPIC_MODEL="claude-3-5-sonnet-20241022"
```

#### 选项 C：OpenAI

```bash
export OPENAI_API_KEY="sk-..."
export LLM_BACKEND="openai"
export OPENAI_MODEL="gpt-4"
```

#### 选项 D：本地 LLM（Ollama）

```bash
export LLM_BACKEND="openai_compatible"
export LLM_BASE_URL="http://localhost:11434/v1"
export LLM_MODEL="llama2"
export LLM_API_KEY="ollama"  # 本地 Ollama 不需要真实的 API key
```

### 3. 配置数据库

```bash
# 使用 SQLite（推荐用于测试）
export DATABASE_URL="sqlite::memory:"

# 或使用 PostgreSQL
export DATABASE_URL="postgres://localhost/ironclaw"
```

## 运行测试

### 运行所有真实 LLM 集成测试

```bash
# 设置环境变量
export LLM_API_KEY="sk-d162b1775e514757b083a2dfe729d6e6"
export LLM_BACKEND="openai_compatible"
export LLM_BASE_URL="https://dashscope.aliyuncs.com/compatible-mode/v1"
export LLM_MODEL="qwen-max"

# 运行测试
cargo test --test e2e_real_llm_job_creation --features libsql -- --nocapture
cargo test --test e2e_real_llm_chat_job_creation --features libsql -- --nocapture
```

### 运行特定测试

```bash
# 运行基础任务创建测试
cargo test --test e2e_real_llm_job_creation test_create_job_via_chat_real_llm --features libsql -- --nocapture

# 运行多任务创建测试
cargo test --test e2e_real_llm_job_creation test_create_multiple_jobs_real_llm --features libsql -- --nocapture

# 运行聊天工作流程测试
cargo test --test e2e_real_llm_chat_job_creation test_chat_job_workflow_with_real_llm --features libsql -- --nocapture
```

### 运行所有测试（单线程）

```bash
cargo test --test e2e_real_llm_job_creation --features libsql -- --nocapture --test-threads=1
cargo test --test e2e_real_llm_chat_job_creation --features libsql -- --nocapture --test-threads=1
```

## 完整示例

### 使用 Qwen API 运行测试

```bash
#!/bin/bash

# 设置环境变量
export LLM_API_KEY="sk-d162b1775e514757b083a2dfe729d6e6"
export LLM_BACKEND="openai_compatible"
export LLM_BASE_URL="https://dashscope.aliyuncs.com/compatible-mode/v1"
export LLM_MODEL="qwen-max"
export DATABASE_URL="sqlite::memory:"
export RUST_LOG="ironclaw=debug"

# 启动后端（可选，如果需要完整的 agent 循环）
# cargo run -- run --no-onboard < /dev/null &
# BACKEND_PID=$!

# 运行测试
echo "Running real LLM integration tests..."
cargo test --test e2e_real_llm_job_creation --features libsql -- --nocapture --test-threads=1

# 清理
# kill $BACKEND_PID 2>/dev/null || true
```

### 使用 Claude API 运行测试

```bash
#!/bin/bash

export ANTHROPIC_API_KEY="sk-ant-..."
export LLM_BACKEND="anthropic"
export ANTHROPIC_MODEL="claude-3-5-sonnet-20241022"
export DATABASE_URL="sqlite::memory:"

cargo test --test e2e_real_llm_chat_job_creation --features libsql -- --nocapture --test-threads=1
```

### 使用本地 Ollama 运行测试

```bash
#!/bin/bash

# 确保 Ollama 正在运行
# ollama serve

export LLM_BACKEND="openai_compatible"
export LLM_BASE_URL="http://localhost:11434/v1"
export LLM_MODEL="llama2"
export LLM_API_KEY="ollama"
export DATABASE_URL="sqlite::memory:"

cargo test --test e2e_real_llm_job_creation --features libsql -- --nocapture --test-threads=1
```

## 测试输出示例

### 成功的测试输出

```
╔════════════════════════════════════════════════════════════╗
║  Test: Create Job via Chat (Real LLM)                      ║
╚════════════════════════════════════════════════════════════╝

🔧 Setting up agent...

📦 Database: sqlite::memory:
🤖 LLM Backend: openai_compatible
  Base URL: https://dashscope.aliyuncs.com/compatible-mode/v1
  Model: qwen-max
✓ Agent setup complete

📤 Step 1: Sending chat message...
   Message: Create a job to analyze system logs
   User: test_user
   Channel: test_channel

✓ Step 2: Verifying job creation...
  Found 1 jobs

  Job 1:
    ID: 550e8400-e29b-41d4-a716-446655440000
    Title: Analyze system logs
    Status: InProgress
    Created: 2024-01-01T00:00:00Z

✓ Step 3: Querying job status...
  Job Status:
    ID: 550e8400-e29b-41d4-a716-446655440000
    Title: Analyze system logs
    Description: Generate a summary report
    Status: InProgress
    User: test_user
    Created: 2024-01-01T00:00:00Z

✅ Test completed successfully!
```

## 故障排除

### 问题 1：LLM API 连接失败

**症状**：
```
Error: Failed to connect to LLM API
```

**解决方案**：
1. 检查 API key 是否正确
2. 检查网络连接
3. 检查 LLM_BASE_URL 是否正确
4. 检查 LLM_MODEL 是否有效

### 问题 2：数据库连接失败

**症状**：
```
Error: Failed to create database
```

**解决方案**：
1. 检查 DATABASE_URL 是否正确
2. 确保 PostgreSQL 正在运行（如果使用 PostgreSQL）
3. 检查数据库权限

### 问题 3：测试超时

**症状**：
```
test timed out after 30s
```

**解决方案**：
1. 增加超时时间
2. 检查 LLM API 响应速度
3. 使用更快的 LLM 模型
4. 检查网络延迟

### 问题 4：任务创建失败

**症状**：
```
Error: Failed to create job
```

**解决方案**：
1. 检查 LLM 是否正确理解了请求
2. 检查 create_job 工具是否可用
3. 检查权限和配置
4. 查看详细的日志输出

## 环境变量参考

### LLM 配置

| 变量 | 说明 | 示例 |
|------|------|------|
| `LLM_BACKEND` | LLM 后端类型 | `openai_compatible`, `anthropic`, `openai` |
| `LLM_API_KEY` | API 密钥 | `sk-...` |
| `LLM_BASE_URL` | API 基础 URL | `https://dashscope.aliyuncs.com/compatible-mode/v1` |
| `LLM_MODEL` | 模型名称 | `qwen-max`, `gpt-4`, `claude-3-5-sonnet-20241022` |
| `ANTHROPIC_API_KEY` | Anthropic API 密钥 | `sk-ant-...` |
| `ANTHROPIC_MODEL` | Anthropic 模型 | `claude-3-5-sonnet-20241022` |
| `OPENAI_API_KEY` | OpenAI API 密钥 | `sk-...` |
| `OPENAI_MODEL` | OpenAI 模型 | `gpt-4` |

### 数据库配置

| 变量 | 说明 | 示例 |
|------|------|------|
| `DATABASE_URL` | 数据库连接字符串 | `sqlite::memory:`, `postgres://localhost/ironclaw` |

### 日志配置

| 变量 | 说明 | 示例 |
|------|------|------|
| `RUST_LOG` | 日志级别 | `ironclaw=debug`, `ironclaw=info` |

## 最佳实践

### 1. 使用单线程运行测试

```bash
cargo test --test e2e_real_llm_job_creation --features libsql -- --nocapture --test-threads=1
```

这确保测试按顺序运行，避免并发问题。

### 2. 启用详细日志

```bash
export RUST_LOG="ironclaw=debug"
cargo test --test e2e_real_llm_job_creation --features libsql -- --nocapture
```

### 3. 使用本地 LLM 进行快速测试

对于快速迭代，使用本地 LLM（如 Ollama）而不是远程 API：

```bash
export LLM_BACKEND="openai_compatible"
export LLM_BASE_URL="http://localhost:11434/v1"
export LLM_MODEL="llama2"
```

### 4. 保存测试输出

```bash
cargo test --test e2e_real_llm_job_creation --features libsql -- --nocapture > test_output.log 2>&1
```

## 相关文件

- `tests/e2e_real_llm_job_creation.rs` - 基础集成测试
- `tests/e2e_real_llm_chat_job_creation.rs` - 高级集成测试
- `JOBS_CREATION_FLOW.md` - 任务创建流程文档
- `CHAT_JOB_CREATION_INTEGRATION_TEST.md` - Mock 数据集成测试文档

## 总结

使用真实 LLM API 的集成测试提供了以下优势：

✅ **真实验证** - 使用实际的 LLM API，而不是 mock 数据
✅ **完整流程** - 验证从聊天消息到任务创建的完整流程
✅ **多 LLM 支持** - 支持 Qwen、Claude、OpenAI 等多个 LLM 提供商
✅ **灵活配置** - 通过环境变量轻松切换 LLM 和数据库
✅ **详细输出** - 清晰的测试输出和日志记录

这些测试确保了通过聊天创建任务的功能在真实环境中正常工作。
