# Desktop Client 快速启动指南

## 架构说明

Desktop Client 现在使用**外部 IronClaw 服务器**，而不是内嵌服务器。这样可以：
- 避免复杂的内嵌启动逻辑
- 更容易调试和维护
- 与主项目保持一致的 API

## 服务架构

```
Desktop Client 前端 (5173) → Tauri → IronClaw 服务器 (38080)
```

## 启动方式

### 方式 1：使用完整启动脚本（推荐）

```bash
./scripts/start-all.sh
```

这会自动启动：
1. PostgreSQL 数据库（端口 5432）
2. IronClaw 服务器（端口 38080）
3. Desktop Client 前端（端口 5173）
4. Tauri 客户端
5. Admin Backend（端口 3000 和 5174）

### 方式 2：手动启动

```bash
# 1. 启动 IronClaw 服务器
export GATEWAY_PORT=38080
export GATEWAY_HOST=127.0.0.1
export GATEWAY_ENABLED=true
cargo run --manifest-path ironclaw/Cargo.toml -- run --no-onboard &

# 2. 等待服务器启动
sleep 10
curl http://localhost:38080/api/health

# 3. 启动 Desktop Client 前端
cd desktop-client/src-ui
npm run dev &

# 4. 启动 Tauri 客户端
cd ..
cargo tauri dev
```

## 验证启动成功

```bash
# 检查 IronClaw 服务器
curl http://localhost:38080/api/health
# 应该返回：{"status":"ok"}

# 检查前端
curl http://localhost:5173
# 应该返回 HTML

# 查看日志
tail -f /tmp/ironclaw-server.log
tail -f /tmp/desktop-frontend.log
tail -f /tmp/tauri.log
```

## 常见问题

### 1. 连接被拒绝（Connection refused）

**症状**：
```
error sending request for url (http://localhost:38080/api/chat/threads): 
error trying to connect: tcp connect error: Connection refused (os error 61)
```

**原因**：IronClaw 服务器未启动

**解决方案**：
```bash
# 检查服务器是否运行
lsof -i :38080

# 如果没有运行，手动启动
export GATEWAY_PORT=38080
export GATEWAY_HOST=127.0.0.1
export GATEWAY_ENABLED=true
cargo run --manifest-path ironclaw/Cargo.toml -- run --no-onboard
```

### 2. Tauri 编译超时

**症状**：
```
⚠️  Tauri 启动超时，继续启动其他服务
```

**原因**：首次编译需要很长时间

**解决方案**：
- 耐心等待（首次编译可能需要 5-10 分钟）
- 查看日志：`tail -f /tmp/tauri.log`
- 预编译：`cd desktop-client && cargo build --no-default-features`

### 3. 端口被占用

**症状**：
```
Address already in use (os error 48)
```

**解决方案**：
```bash
# 查找占用端口的进程
lsof -i :38080
lsof -i :5173

# 杀死进程
kill -9 <PID>

# 或使用启动脚本的清理功能
./scripts/start-all.sh  # 会自动清理旧进程
```

## 环境变量配置

Desktop Client 需要以下环境变量：

```bash
# IronClaw 服务器配置
GATEWAY_PORT=38080
GATEWAY_HOST=127.0.0.1
GATEWAY_ENABLED=true

# LLM 配置（从 .env 继承）
LLM_BACKEND=openai_compatible
LLM_BASE_URL=https://dashscope.aliyuncs.com/compatible-mode/v1
LLM_MODEL=qwen-max
LLM_API_KEY=sk-...
```

## 开发工作流

1. **启动所有服务**：
   ```bash
   ./scripts/start-all.sh
   ```

2. **开发前端**：
   - 前端代码在 `desktop-client/src-ui/`
   - 修改后自动热重载（Vite）

3. **开发后端**：
   - Rust 代码在 `desktop-client/src/`
   - 修改后需要重启 Tauri：`Ctrl+C` 然后重新运行

4. **查看日志**：
   ```bash
   tail -f /tmp/ironclaw-server.log  # IronClaw 服务器
   tail -f /tmp/desktop-frontend.log # 前端
   tail -f /tmp/tauri.log            # Tauri
   ```

5. **停止所有服务**：
   - 按 `Ctrl+C` 停止启动脚本
   - 或手动杀死进程

## 测试

```bash
# 运行单元测试
cd desktop-client
cargo test

# 运行 E2E 测试
npm run test:e2e
```

## 故障排除

如果遇到问题，请查看：
- `admin-backend/TROUBLESHOOTING.md` - 详细的故障排除指南
- `/tmp/ironclaw-server.log` - IronClaw 服务器日志
- `/tmp/tauri.log` - Tauri 客户端日志
- `/tmp/desktop-frontend.log` - 前端日志

## 参考文档

- `AGENTS.md` - 开发规则和最佳实践
- `admin-backend/TROUBLESHOOTING.md` - 故障排除指南
- `scripts/start-all.sh` - 启动脚本源码
