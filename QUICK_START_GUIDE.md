# 快速启动指南

## 一句话总结

设置 API 密钥，然后运行启动脚本即可。

---

## 3 步启动

### 步骤 1: 设置 Qwen API 密钥

```bash
export LLM_API_KEY="sk-d162b1775e514757b083a2dfe729d6e6"
```

### 步骤 2: 运行启动脚本

```bash
bash scripts/start-all.sh
```

脚本会自动：
- ✅ 清理旧进程
- ✅ 启动后端服务
- ✅ 启动前端开发服务器
- ✅ 启动 Tauri 桌面客户端
- ✅ 显示访问 URL

### 步骤 3: 使用应用

**浏览器版本**: 访问 http://localhost:5173

**桌面客户端**: Tauri 应用会自动启动

---

## 单独启动 Tauri 客户端

如果后端和前端已经在运行，只需启动 Tauri 客户端：

```bash
bash scripts/test-tauri-token.sh
```

这个脚本会：
- 从后端日志提取认证令牌
- 设置环境变量 `GATEWAY_AUTH_TOKEN`
- 启动 Tauri 桌面客户端

---

## 测试聊天功能

1. 在输入框输入消息（例如："你好，Qwen！")
2. 点击发送按钮
3. 应该能收到 Qwen 的中文回复

---

## 常见问题

### Q: Permission denied

**A**: 添加执行权限
```bash
chmod +x scripts/start-all.sh
```

### Q: LLM_API_KEY 环境变量未设置

**A**: 设置 API 密钥
```bash
export LLM_API_KEY="sk-d162b1775e514757b083a2dfe729d6e6"
```

### Q: 后端启动失败

**A**: 查看后端日志
```bash
tail -50 /tmp/backend.log
```

### Q: 前端无法连接到后端

**A**: 检查后端是否运行
```bash
curl http://localhost:3000/api/health
```

### Q: Tauri 客户端报 401 错误

**A**: 认证令牌不匹配，使用以下方法解决：

**方法 1（推荐）**: 使用启动脚本
```bash
bash scripts/start-all.sh
```

**方法 2**: 单独启动 Tauri
```bash
bash scripts/test-tauri-token.sh
```

**方法 3**: 手动设置令牌
```bash
# 1. 从后端日志获取 token
grep "token=" /tmp/backend.log | tail -1

# 2. 设置环境变量
export GATEWAY_AUTH_TOKEN="your-token-here"

# 3. 启动 Tauri
cd desktop-client
cargo tauri dev
```

### Q: 消息发送后没有响应

**A**: 可能是以下原因：
1. 后端没有正确启动
2. Qwen API 密钥无效
3. 网络连接问题

查看后端日志：
```bash
tail -100 /tmp/backend.log
```

---

## 停止服务

按 `Ctrl+C` 停止所有服务。

---

## 完整的启动命令

```bash
export LLM_API_KEY="sk-d162b1775e514757b083a2dfe729d6e6" && bash scripts/start-all.sh
```

---

## 详细文档

- `QWEN_QUICK_START.md` - 详细的启动指南
- `BACKEND_STARTUP_ISSUES_AND_SOLUTIONS.md` - 问题和解决方案
- `QWEN_DIAGNOSTIC_REPORT.md` - 诊断报告

