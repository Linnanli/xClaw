# x-claw 快速启动指南

## 🚀 快速开始（3 步）

### 1️⃣ 配置 Qwen API 密钥

编辑 `.env` 文件：

```bash
nano .env
```

找到这一行：
```bash
LLM_API_KEY=sk-your-qwen-api-key-here  # ⚠️ 请替换为你的实际 API 密钥
```

替换为你的实际 API 密钥：
```bash
LLM_API_KEY=sk-abc123...  # 你的实际密钥
```

**获取 API 密钥**: https://dashscope.console.aliyun.com/api-key

### 2️⃣ 启动服务

```bash
bash scripts/start-all.sh
```

这个脚本会自动：
- ✅ 检查依赖（Rust, Node.js, npm）
- ✅ 加载配置（从 .env 和 ~/.ironclaw/.env）
- ✅ 清理旧进程和端口
- ✅ 启动后端服务（端口 3000）
- ✅ 启动前端服务（端口 5173）
- ✅ 启动 Tauri 桌面客户端
- ✅ 自动打开浏览器

### 3️⃣ 访问应用

启动成功后，浏览器会自动打开：

```
http://localhost:5173?token=<自动生成的令牌>
```

或者手动访问：
- **前端**: http://localhost:5173
- **后端 API**: http://localhost:3000
- **桌面客户端**: 自动启动

## 📋 服务管理

### 查看服务状态

```bash
# 查看后端日志
tail -f /tmp/backend.log

# 查看运行中的进程
ps aux | grep -E "cargo|ironclaw|vite"

# 检查端口占用
lsof -i :3000  # 后端
lsof -i :5173  # 前端
lsof -i :8080  # Webhook
```

### 停止服务

按 `Ctrl+C` 停止启动脚本，所有服务会自动清理。

或者手动停止：

```bash
# 停止所有相关进程
pkill -f "cargo run"
pkill -f "vite"
pkill -f "tauri"

# 清理端口
lsof -i :3000 | grep -v COMMAND | awk '{print $2}' | xargs kill -9
lsof -i :5173 | grep -v COMMAND | awk '{print $2}' | xargs kill -9
```

### 重启服务

```bash
# 停止旧服务
pkill -f "cargo run"

# 重新启动
bash scripts/start-all.sh
```

## 🔧 配置说明

### 当前配置

**项目本地配置** (`./.env`):
- LLM 提供商: **Qwen** (qwen-max)
- 数据库: 继承全局配置
- 优先级: 高（覆盖全局配置）

**全局配置** (`~/.ironclaw/.env`):
- LLM 提供商: **Anthropic Claude**
- 数据库: libsql (本地)
- 优先级: 低（被项目配置覆盖）

### 切换 LLM 提供商

编辑 `.env` 文件，注释掉 Qwen 配置，启用其他提供商：

```bash
# 使用 Anthropic Claude
LLM_BACKEND=anthropic
ANTHROPIC_API_KEY=sk-ant-...
ANTHROPIC_MODEL=claude-3-5-sonnet-20241022

# 使用 OpenAI
# LLM_BACKEND=openai
# OPENAI_API_KEY=sk-...
# OPENAI_MODEL=gpt-4

# 使用 Ollama (本地)
# LLM_BACKEND=ollama
# OLLAMA_MODEL=llama3.2
# OLLAMA_BASE_URL=http://localhost:11434
```

## 🐛 故障排查

### 问题 1: 后端启动失败

**症状**: 健康检查失败，无法连接到 http://localhost:3000

**解决方案**:
```bash
# 查看后端日志
tail -100 /tmp/backend.log

# 检查 API 密钥是否正确
grep LLM_API_KEY .env

# 检查端口是否被占用
lsof -i :3000
```

### 问题 2: API 密钥错误

**症状**: 后端日志显示认证失败

**解决方案**:
```bash
# 验证 API 密钥格式
# Qwen: sk-开头
# Anthropic: sk-ant-开头
# OpenAI: sk-开头

# 重新配置
nano .env
```

### 问题 3: 前端无法连接后端

**症状**: 前端显示连接错误

**解决方案**:
```bash
# 检查后端是否运行
curl http://localhost:3000/api/health

# 检查认证令牌
grep "gateway.*http" /tmp/backend.log

# 重启服务
bash scripts/start-all.sh
```

### 问题 4: 编译错误

**症状**: cargo build 失败

**解决方案**:
```bash
# 清理构建缓存
cargo clean

# 更新依赖
cargo update

# 重新构建
cargo build
```

### 问题 5: 端口被占用

**症状**: 启动脚本报告端口已被占用

**解决方案**:
```bash
# 查找占用端口的进程
lsof -i :3000
lsof -i :5173
lsof -i :8080

# 杀死进程
kill -9 <PID>

# 或使用启动脚本的清理功能
bash scripts/start-all.sh  # 会自动清理端口
```

## 📚 更多文档

- **详细配置指南**: `CONFIG_GUIDE.md`
- **配置总结**: `CONFIGURATION_SUMMARY.md`
- **环境变量示例**: `.env.example`
- **项目 README**: `README.md`

## 💡 提示

1. **API 密钥安全**: 
   - `.env` 文件已在 `.gitignore` 中
   - 不要将 API 密钥提交到 Git
   - 不要分享你的 API 密钥

2. **配置优先级**:
   ```
   环境变量 > ./.env > ~/.ironclaw/.env > 数据库 > 默认值
   ```

3. **日志查看**:
   ```bash
   # 实时查看后端日志
   tail -f /tmp/backend.log
   
   # 查看启动日志
   cat /tmp/startup.log
   ```

4. **性能优化**:
   - 首次启动需要编译，可能需要 5-10 分钟
   - 后续启动会快很多（使用缓存）
   - 使用 `cargo build --release` 构建优化版本

## 🎉 开始使用

配置完成后，你可以：

1. **通过浏览器使用**: http://localhost:5173
2. **通过桌面客户端使用**: 自动启动的 Tauri 应用
3. **通过 API 使用**: http://localhost:3000/api/*
4. **通过 CLI 使用**: `cargo run -- run`

祝你使用愉快！🚀
