# 完整启动说明

**最后更新**: 2026-03-16

---

## 一句话启动

```bash
export LLM_API_KEY="sk-d162b1775e514757b083a2dfe729d6e6" && bash scripts/start-all.sh
```

---

## 详细步骤

### 步骤 1: 打开终端

在项目根目录打开终端。

### 步骤 2: 设置 API 密钥

```bash
export LLM_API_KEY="sk-d162b1775e514757b083a2dfe729d6e6"
```

### 步骤 3: 运行启动脚本

```bash
bash scripts/start-all.sh
```

### 步骤 4: 等待启动完成

脚本会自动：
- ✅ 清理旧进程
- ✅ 启动后端服务
- ✅ 启动前端 Web 服务
- ✅ 启动 Tauri 客户端
- ✅ 打开浏览器

---

## 启动输出

```
✅ 所有服务已启动

后端服务:
  URL: http://localhost:3000
  PID: 19233
  日志: tail -f /tmp/backend.log

前端 Web 服务:
  URL: http://localhost:5173
  PID: 19234

Tauri 客户端:
  PID: 19235
  状态: 运行中

前端 URL（带令牌）:
  http://localhost:5173?token=36c1a0275ae2708954a402871a27255bcecf9aa24f7542a43ff0c5e1f651b19b

💡 提示: 使用上面的 URL 访问前端，令牌会自动保存到本地存储

🌐 正在打开浏览器...

停止服务: 按 Ctrl+C
```

---

## 访问应用

### 方式 1: 浏览器 Web 界面

浏览器会自动打开：
```
http://localhost:5173?token=...
```

### 方式 2: Tauri 桌面应用

Tauri 应用窗口会自动打开。

### 方式 3: 手动访问

如果浏览器没有自动打开，手动访问：
```
http://localhost:5173?token=36c1a0275ae2708954a402871a27255bcecf9aa24f7542a43ff0c5e1f651b19b
```

---

## 使用应用

### 聊天功能

1. 在聊天框输入消息
2. 点击发送按钮
3. 等待 Qwen 的回复

### 其他功能

- 📝 **记忆** - 查看和管理记忆
- 💼 **任务** - 查看和管理任务
- 📅 **日程** - 查看和管理日程
- 🧩 **扩展** - 安装和管理扩展
- ⚡ **技能** - 查看和管理技能
- 📋 **日志** - 查看系统日志
- ⚙️ **设置** - 应用设置

---

## 停止应用

### 方式 1: 按 Ctrl+C

在终端中按 `Ctrl+C` 停止所有服务。

### 方式 2: 关闭应用窗口

关闭 Tauri 应用窗口。

### 方式 3: 关闭浏览器标签

关闭浏览器标签页。

---

## 故障排除

### Q: 脚本权限不足

```bash
chmod +x scripts/start-all.sh
```

### Q: 后端启动失败

查看后端日志：
```bash
tail -50 /tmp/backend.log
```

### Q: 前端无法连接

检查后端是否运行：
```bash
curl http://localhost:3000/api/health
```

### Q: Tauri 窗口没有打开

可能是以下原因：
1. 前端开发服务器未启动
2. 后端服务未运行
3. 端口被占用

### Q: 消息发送后没有响应

可能是以下原因：
1. Qwen API 密钥无效
2. 网络连接问题
3. 后端日志中有错误

查看后端日志：
```bash
tail -100 /tmp/backend.log | grep -i error
```

---

## 环境要求

### 必需

- Rust 1.92+
- Node.js 16+
- npm 或 yarn
- Qwen API 密钥

### 可选

- Xcode（macOS）
- Visual Studio Build Tools（Windows）

---

## 相关文档

- `QUICK_START_GUIDE.md` - 快速启动指南
- `QWEN_QUICK_START.md` - Qwen 配置指南
- `TOKEN_MANAGEMENT_GUIDE.md` - 令牌管理指南
- `TAURI_CLIENT_STARTUP_GUIDE.md` - Tauri 启动指南
- `BACKEND_STARTUP_ISSUES_AND_SOLUTIONS.md` - 问题和解决方案

---

## 常用命令

```bash
# 启动所有服务
bash scripts/start-all.sh

# 查看后端日志
tail -f /tmp/backend.log

# 查看后端错误
tail -100 /tmp/backend.log | grep -i error

# 停止所有服务
pkill -f "cargo run"
pkill -f "npm run dev"
pkill -f "cargo tauri dev"

# 清理旧进程
rm -f ~/.ironclaw/ironclaw.pid

# 清理端口
lsof -i :3000 | grep -v COMMAND | awk '{print $2}' | xargs kill -9
lsof -i :5173 | grep -v COMMAND | awk '{print $2}' | xargs kill -9
```

---

## 下一步

1. ✅ 设置 `LLM_API_KEY` 环境变量
2. ✅ 运行 `bash scripts/start-all.sh`
3. ✅ 等待所有服务启动
4. ✅ 浏览器会自动打开
5. ✅ 开始使用应用

