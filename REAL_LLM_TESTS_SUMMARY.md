# 真实 LLM API 集成测试总结

## 概述

已创建使用真实 LLM API（而不是 mock 数据）的集成测试，用于验证通过聊天创建任务的完整流程。

## 创建的文件

### 1. 测试文件

#### `tests/e2e_real_llm_job_creation.rs`
基础集成测试，验证任务创建和管理功能。

**测试用例**：
- `test_create_job_via_chat_real_llm` - 通过聊天创建单个任务
- `test_create_multiple_jobs_real_llm` - 创建多个任务
- `test_job_state_transitions_real_llm` - 验证任务状态转换

**特点**：
- ✅ 使用真实 LLM API
- ✅ 验证任务创建
- ✅ 验证任务列表
- ✅ 验证任务状态管理

#### `tests/e2e_real_llm_chat_job_creation.rs`
高级集成测试，验证完整的聊天工作流程。

**测试用例**：
- `test_chat_create_job_with_real_llm` - 通过聊天创建任务
- `test_chat_job_workflow_with_real_llm` - 完整的任务工作流程（创建、列出、查询、取消）
- `test_llm_job_understanding_with_real_llm` - LLM 对任务请求的理解

**特点**：
- ✅ 完整的 agent 设置
- ✅ 真实的聊天交互
- ✅ 多步骤工作流程
- ✅ 详细的输出日志

### 2. 文档

#### `REAL_LLM_INTEGRATION_TEST_GUIDE.md`
完整的集成测试指南，包含：
- 前置条件设置
- 多个 LLM 提供商的配置（Qwen、Claude、OpenAI、Ollama）
- 运行测试的详细步骤
- 故障排除指南
- 最佳实践

#### `REAL_LLM_TESTS_SUMMARY.md`（本文件）
快速参考和总结

### 3. 脚本

#### `scripts/run-real-llm-tests.sh`
自动化测试运行脚本，支持多个 LLM 提供商。

**用法**：
```bash
./scripts/run-real-llm-tests.sh [qwen|claude|openai|ollama]
```

**特点**：
- ✅ 自动配置 LLM
- ✅ 验证环境变量
- ✅ 运行所有测试
- ✅ 彩色输出

## 快速开始

### 1. 使用 Qwen API

```bash
# 设置环境变量
export LLM_API_KEY="sk-d162b1775e514757b083a2dfe729d6e6"
export LLM_BACKEND="openai_compatible"
export LLM_BASE_URL="https://dashscope.aliyuncs.com/compatible-mode/v1"
export LLM_MODEL="qwen-max"

# 运行测试
./scripts/run-real-llm-tests.sh qwen
```

### 2. 使用 Claude API

```bash
# 设置环境变量
export ANTHROPIC_API_KEY="sk-ant-..."

# 运行测试
./scripts/run-real-llm-tests.sh claude
```

### 3. 使用 OpenAI API

```bash
# 设置环境变量
export OPENAI_API_KEY="sk-..."

# 运行测试
./scripts/run-real-llm-tests.sh openai
```

### 4. 使用本地 Ollama

```bash
# 启动 Ollama
ollama serve

# 在另一个终端运行测试
./scripts/run-real-llm-tests.sh ollama
```

## 测试流程

### 基础流程

```
1. 设置 LLM 提供商
   ↓
2. 初始化 Agent
   ↓
3. 创建任务
   ↓
4. 验证任务创建
   ↓
5. 查询任务状态
   ↓
6. 验证任务列表
   ↓
7. 更新任务状态
   ↓
8. 验证状态转换
```

### 聊天工作流程

```
1. 设置 Agent
   ↓
2. 发送聊天消息："Create a job to analyze logs"
   ↓
3. LLM 处理消息
   ↓
4. LLM 决定调用 create_job 工具
   ↓
5. 任务被创建
   ↓
6. 验证任务出现在列表中
   ↓
7. 查询任务状态
   ↓
8. 验证任务详情
```

## 支持的 LLM 提供商

| 提供商 | 后端 | 配置 | 成本 |
|--------|------|------|------|
| Qwen | openai_compatible | API Key | 付费 |
| Claude | anthropic | API Key | 付费 |
| OpenAI | openai | API Key | 付费 |
| Ollama | openai_compatible | 本地 | 免费 |

## 环境变量

### Qwen 配置

```bash
export LLM_API_KEY="sk-..."
export LLM_BACKEND="openai_compatible"
export LLM_BASE_URL="https://dashscope.aliyuncs.com/compatible-mode/v1"
export LLM_MODEL="qwen-max"
```

### Claude 配置

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
export LLM_BACKEND="anthropic"
export ANTHROPIC_MODEL="claude-3-5-sonnet-20241022"
```

### OpenAI 配置

```bash
export OPENAI_API_KEY="sk-..."
export LLM_BACKEND="openai"
export OPENAI_MODEL="gpt-4"
```

### Ollama 配置

```bash
export LLM_BACKEND="openai_compatible"
export LLM_BASE_URL="http://localhost:11434/v1"
export LLM_MODEL="llama2"
export LLM_API_KEY="ollama"
```

### 数据库配置

```bash
export DATABASE_URL="sqlite::memory:"  # 内存数据库（推荐用于测试）
# 或
export DATABASE_URL="postgres://localhost/ironclaw"  # PostgreSQL
```

### 日志配置

```bash
export RUST_LOG="ironclaw=debug"  # 详细日志
export RUST_LOG="ironclaw=info"   # 信息日志
```

## 运行测试

### 运行所有测试

```bash
# 使用 Qwen
export LLM_API_KEY="sk-..."
./scripts/run-real-llm-tests.sh qwen

# 或手动运行
cargo test --test e2e_real_llm_job_creation --features libsql -- --nocapture --test-threads=1
cargo test --test e2e_real_llm_chat_job_creation --features libsql -- --nocapture --test-threads=1
```

### 运行特定测试

```bash
# 基础任务创建
cargo test --test e2e_real_llm_job_creation test_create_job_via_chat_real_llm --features libsql -- --nocapture

# 多任务创建
cargo test --test e2e_real_llm_job_creation test_create_multiple_jobs_real_llm --features libsql -- --nocapture

# 聊天工作流程
cargo test --test e2e_real_llm_chat_job_creation test_chat_job_workflow_with_real_llm --features libsql -- --nocapture
```

## 测试输出示例

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

## 优势

### 相比 Mock 数据测试

✅ **真实验证** - 使用实际的 LLM API，而不是预录制的响应
✅ **完整流程** - 验证从聊天消息到任务创建的完整流程
✅ **多 LLM 支持** - 支持多个 LLM 提供商
✅ **灵活配置** - 通过环境变量轻松切换
✅ **生产就绪** - 测试与生产环境相同的代码路径

### 缺点

❌ **成本** - 使用付费 API 会产生成本
❌ **速度** - 比 mock 测试慢
❌ **依赖性** - 依赖外部 API 的可用性
❌ **不确定性** - LLM 的响应可能不确定

## 最佳实践

### 1. 使用单线程运行

```bash
cargo test --test e2e_real_llm_job_creation --features libsql -- --nocapture --test-threads=1
```

### 2. 启用详细日志

```bash
export RUST_LOG="ironclaw=debug"
```

### 3. 使用本地 LLM 进行快速迭代

```bash
./scripts/run-real-llm-tests.sh ollama
```

### 4. 保存测试输出

```bash
./scripts/run-real-llm-tests.sh qwen > test_output.log 2>&1
```

### 5. 定期运行测试

建议在以下情况下运行测试：
- 修改 agent 代码
- 修改任务创建逻辑
- 升级 LLM 模型
- 集成新的 LLM 提供商

## 故障排除

### 问题：LLM API 连接失败

```
Error: Failed to connect to LLM API
```

**解决方案**：
1. 检查 API key 是否正确
2. 检查网络连接
3. 检查 LLM_BASE_URL 是否正确
4. 检查 LLM_MODEL 是否有效

### 问题：任务创建失败

```
Error: Failed to create job
```

**解决方案**：
1. 检查 LLM 是否正确理解了请求
2. 检查 create_job 工具是否可用
3. 查看详细的日志输出
4. 尝试使用不同的 LLM 模型

### 问题：测试超时

```
test timed out after 30s
```

**解决方案**：
1. 增加超时时间
2. 检查 LLM API 响应速度
3. 使用更快的 LLM 模型
4. 检查网络延迟

## 相关文件

- `tests/e2e_real_llm_job_creation.rs` - 基础集成测试
- `tests/e2e_real_llm_chat_job_creation.rs` - 高级集成测试
- `REAL_LLM_INTEGRATION_TEST_GUIDE.md` - 完整指南
- `scripts/run-real-llm-tests.sh` - 自动化脚本
- `JOBS_CREATION_FLOW.md` - 任务创建流程
- `CHAT_JOB_CREATION_INTEGRATION_TEST.md` - Mock 数据测试

## 总结

✅ **已创建真实 LLM API 集成测试**

### 创建的内容

1. **两个测试文件**
   - 基础任务创建测试
   - 高级聊天工作流程测试

2. **完整的文档**
   - 详细的设置指南
   - 多个 LLM 提供商的配置
   - 故障排除指南

3. **自动化脚本**
   - 支持多个 LLM 提供商
   - 自动环境配置
   - 彩色输出

### 下一步

1. 设置 LLM API 密钥
2. 运行测试脚本
3. 验证任务创建功能
4. 根据需要调整测试

### 命令参考

```bash
# 快速开始
export LLM_API_KEY="sk-..."
./scripts/run-real-llm-tests.sh qwen

# 或手动运行
cargo test --test e2e_real_llm_job_creation --features libsql -- --nocapture --test-threads=1
```
