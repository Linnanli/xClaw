# 桌面客户端设置和故障排除指南

## 🚀 完整启动流程

### 第一步：启动主项目服务（Web Gateway API）

桌面客户端需要连接到主项目的 Web Gateway API，该 API 默认运行在 `http://localhost:8000`。

```bash
# 1. 进入项目根目录
cd /path/to/ironclaw

# 2. 启动主项目服务
cargo run --release -- service

# 或者使用开发模式（更快的编译）
cargo run -- service
```

**预期输出：**
```
[INFO] Starting IronClaw service...
[INFO] Web Gateway listening on http://localhost:8000
[INFO] Database connected
```

### 第二步：启动前端开发服务器（Vite）

在新的终端窗口中：

```bash
# 1. 进入桌面客户端目录
cd desktop-client/src-ui

# 2. 启动 Vite 开发服务器
npm run dev
```

**预期输出：**
```
  VITE v6.3.5  ready in XXX ms

  ➜  Local:   http://localhost:5173/
  ➜  press h to show help
```

### 第三步：启动 Tauri 应用

在第三个终端窗口中：

```bash
# 1. 进入桌面客户端目录
cd desktop-client

# 2. 启动 Tauri 应用
cargo tauri dev
```

**预期输出：**
```
[INFO] Compiling desktop-client...
[INFO] Tauri app started
[INFO] Connected to Vite dev server
```

## ✅ 验证一切正常

### 1. 检查后端服务

```bash
# 在浏览器中访问
curl http://localhost:8000/health

# 或在浏览器中打开
http://localhost:8000/health
```

**预期响应：**
```json
{"status": "ok"}
```

### 2. 检查前端连接

打开浏览器开发者工具（F12），查看：
- **Console 标签页** - 应该没有错误
- **Network 标签页** - 应该能看到对 `http://localhost:8000` 的请求

### 3. 测试聊天功能

1. 打开应用
2. 点击"新建对话"
3. 输入消息
4. 点击"发送"
5. 在浏览器开发者工具的 Network 标签页中应该能看到请求

## 🐛 常见问题和解决方案

### 问题 1：聊天发送没有反应

**症状：**
- 点击发送按钮没有任何反应
- 浏览器开发者工具中没有网络请求

**原因：**
- 后端服务未运行
- API 基础 URL 配置不正确
- 网络连接问题

**解决方案：**

1. **检查后端服务是否运行**
   ```bash
   # 在终端中检查
   curl http://localhost:8000/health
   
   # 如果返回 Connection refused，说明服务未运行
   # 请按照"第一步"启动主项目服务
   ```

2. **检查 API 基础 URL 配置**
   - 打开 `desktop-client/src-ui/src/app/utils/tauri.ts`
   - 查看 `ApiClient` 的初始化
   - 确保 `base_url` 是 `http://localhost:8000`

3. **检查网络连接**
   - 打开浏览器开发者工具（F12）
   - 切换到 Network 标签页
   - 点击发送按钮
   - 查看是否有请求发出
   - 如果没有请求，检查 JavaScript 控制台是否有错误

### 问题 2：后端服务启动失败

**症状：**
```
error: could not compile `ironclaw`
```

**解决方案：**

1. **清除编译缓存**
   ```bash
   cargo clean
   ```

2. **检查依赖**
   ```bash
   cargo check
   ```

3. **查看详细错误**
   ```bash
   cargo run -- service 2>&1 | head -50
   ```

### 问题 3：前端无法连接到后端

**症状：**
- 浏览器控制台显示 CORS 错误
- 网络请求返回 403 或 CORS 错误

**解决方案：**

1. **检查 CORS 配置**
   - 后端应该允许来自 `http://localhost:5173` 的请求
   - 检查 `src/channels/web/` 中的 CORS 配置

2. **检查防火墙**
   - 确保端口 8000 和 5173 未被防火墙阻止

### 问题 4：Tauri 应用无法启动

**症状：**
```
error: failed to start tauri app
```

**解决方案：**

1. **检查 Vite 服务器是否运行**
   ```bash
   # 在浏览器中访问
   http://localhost:5173
   ```

2. **清除 Tauri 缓存**
   ```bash
   cd desktop-client
   rm -rf src-tauri/target
   cargo tauri dev
   ```

3. **检查 Tauri 配置**
   - 打开 `desktop-client/tauri.conf.json`
   - 确保 `devUrl` 是 `http://localhost:5173`

## 📊 完整的三终端启动流程

为了方便，这是完整的启动流程：

**终端 1 - 启动后端服务：**
```bash
cd /path/to/ironclaw
cargo run -- service
```

**终端 2 - 启动前端开发服务器：**
```bash
cd /path/to/ironclaw/desktop-client/src-ui
npm run dev
```

**终端 3 - 启动 Tauri 应用：**
```bash
cd /path/to/ironclaw/desktop-client
cargo tauri dev
```

## 🔍 调试技巧

### 1. 查看后端日志

后端服务的日志会显示在终端 1 中。查看是否有错误或警告。

### 2. 查看前端日志

前端日志显示在两个地方：
- **终端 2** - Vite 开发服务器的输出
- **浏览器控制台** - 按 F12 打开

### 3. 查看网络请求

打开浏览器开发者工具（F12），切换到 Network 标签页：
- 查看请求 URL 是否正确
- 查看响应状态码（200 表示成功）
- 查看响应内容

### 4. 启用详细日志

在后端启动时启用详细日志：
```bash
RUST_LOG=debug cargo run -- service
```

## 🎯 验证清单

启动完成后，检查以下项目：

- [ ] 后端服务运行在 `http://localhost:8000`
- [ ] 前端开发服务器运行在 `http://localhost:5173`
- [ ] Tauri 应用窗口已打开
- [ ] 浏览器控制台没有错误
- [ ] 可以创建新对话
- [ ] 可以发送消息
- [ ] 消息显示在聊天窗口中
- [ ] 可以查看消息历史

## 📝 设置页面实现

**当前状态：** 设置按钮已实现，但设置页面功能不完整

**待实现的功能：**
1. 创建 `SettingsTab.tsx` 组件
2. 实现以下设置：
   - API 基础 URL 配置
   - 主题选择
   - 语言选择
   - 快捷键配置
   - 日志级别设置
   - 缓存清除

**快速修复：** 如果需要立即使用，可以在 `MainApp.tsx` 中临时禁用设置按钮，或创建一个简单的设置页面。

## 🚀 下一步

1. **实现设置页面** - 允许用户配置 API URL 和其他选项
2. **实现记忆标签页** - 完整的文件浏览和编辑功能
3. **实现任务标签页** - 完整的任务管理功能
4. **添加错误处理** - 更好的错误提示和恢复机制
5. **性能优化** - 虚拟滚动、缓存等

## 📞 获取帮助

如果遇到问题：

1. 检查本文档中的故障排除部分
2. 查看浏览器控制台和终端输出中的错误信息
3. 查看 `QUICK_START.md` 了解基本开发流程
4. 查看 `DESKTOP_CLIENT_BUILD_GUIDE.md` 了解构建说明
