# x-claw 配置指南

## 配置文件优先级

x-claw 使用多层配置系统，优先级从高到低：

```
1. 显式环境变量 (export LLM_API_KEY=...)
2. 项目本地 ./.env (项目根目录) ⭐ 推荐用于项目特定配置
3. 全局 ~/.ironclaw/.env (用户主目录) - 跨项目共享
4. 数据库配置表
5. 自动检测/默认值
```

## 配置文件说明

### 1. 项目本地配置 `./.env` (推荐)

**位置**: 项目根目录  
**用途**: 项目特定的配置，覆盖全局配置  
**优点**: 
- 不同项目可以使用不同的 LLM 提供商
- 团队协作时可以有统一的开发环境配置
- 不会影响其他 ironclaw 项目

**示例**:
```bash
# x-claw 项目使用 Qwen
LLM_BACKEND=openai_compatible
LLM_BASE_URL=https://dashscope.aliyuncs.com/compatible-mode/v1
LLM_MODEL=qwen-max
LLM_API_KEY=sk-your-qwen-api-key-here
```

### 2. 全局配置 `~/.ironclaw/.env`

**位置**: `~/.ironclaw/.env`  
**用途**: 跨项目共享的配置  
**优点**:
- 所有 ironclaw 项目共享同一套配置
- 适合个人开发者
- 引导层配置（数据库连接等）

**当前配置**:
```bash
DATABASE_BACKEND="libsql"
LIBSQL_PATH="/Users/nallylin/.ironclaw/ironclaw.db"
LLM_BACKEND="anthropic"
ANTHROPIC_MODEL="claude-3-5-sonnet-20241022"
ANTHROPIC_API_KEY="sk-ant-..."
```

### 3. ~~配置文件 `~/.ironclaw/config.toml`~~ (已废弃)

**位置**: `~/.ironclaw/config.toml`  
**状态**: ⚠️ 已废弃，建议删除  
**说明**: 
- 旧版配置格式，优先级低于 `.env` 文件
- 代码中仍支持读取，但仅用于向后兼容
- **推荐使用 `.env` 文件代替**

**迁移建议**:
```bash
# 备份旧配置
cp ~/.ironclaw/config.toml ~/.ironclaw/config.toml.backup

# 删除旧配置（已完成）
rm ~/.ironclaw/config.toml

# 所有配置现在都在 .env 文件中
```

## 推荐配置方案

### 方案 A: 项目特定配置（当前推荐）

1. 在项目根目录创建 `.env` 文件
2. 配置项目特定的 LLM 提供商
3. 全局配置保持不变，用于其他项目

```bash
# 编辑项目配置
nano .env

# 添加 Qwen 配置
LLM_BACKEND=openai_compatible
LLM_BASE_URL=https://dashscope.aliyuncs.com/compatible-mode/v1
LLM_MODEL=qwen-max
LLM_API_KEY=sk-your-qwen-api-key-here
```

### 方案 B: 全局配置

1. 修改 `~/.ironclaw/.env`
2. 所有项目使用相同的配置

```bash
# 编辑全局配置
nano ~/.ironclaw/.env
```

## 支持的 LLM 提供商

### Anthropic (Claude)
```bash
LLM_BACKEND=anthropic
ANTHROPIC_MODEL=claude-3-5-sonnet-20241022
ANTHROPIC_API_KEY=sk-ant-...
```

### Qwen (阿里云)
```bash
LLM_BACKEND=openai_compatible
LLM_BASE_URL=https://dashscope.aliyuncs.com/compatible-mode/v1
LLM_MODEL=qwen-max
LLM_API_KEY=sk-...
```
获取 API 密钥: https://dashscope.console.aliyun.com/api-key

### OpenAI
```bash
LLM_BACKEND=openai
OPENAI_API_KEY=sk-...
OPENAI_MODEL=gpt-4
```

### NEAR AI
```bash
LLM_BACKEND=nearai
NEARAI_MODEL=zai-org/GLM-5-FP8
NEARAI_BASE_URL=https://private.near.ai
```

### Ollama (本地)
```bash
LLM_BACKEND=ollama
OLLAMA_MODEL=llama3.2
OLLAMA_BASE_URL=http://localhost:11434
```

## 验证配置

```bash
# 查看当前配置
cat .env
cat ~/.ironclaw/.env

# 测试后端启动
cargo run -- run --no-onboard

# 查看启动日志中的 LLM 配置
tail -f /tmp/backend.log | grep -i "model\|backend"
```

## 常见问题

### Q: 为什么有两个配置文件？
A: 
- `~/.ironclaw/.env` 是全局配置，所有项目共享
- `./.env` 是项目本地配置，优先级更高，可以覆盖全局配置

### Q: 我应该使用哪个？
A: 
- 个人开发：使用全局配置 `~/.ironclaw/.env`
- 团队协作/多项目：使用项目本地配置 `./.env`

### Q: 配置文件会被提交到 Git 吗？
A: 不会，`.env` 已经在 `.gitignore` 中，不会被提交

### Q: 如何切换 LLM 提供商？
A: 修改 `.env` 文件中的 `LLM_BACKEND` 和相关配置，然后重启服务

## 安全提示

⚠️ **永远不要将包含 API 密钥的 `.env` 文件提交到 Git！**

- `.env` 文件已在 `.gitignore` 中
- 使用 `.env.example` 作为模板分享给团队
- API 密钥应该保密，不要分享给他人
