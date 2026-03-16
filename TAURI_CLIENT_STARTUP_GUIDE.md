# Tauri 客户端启动指南

## 问题

启动脚本中的 Tauri 客户端启动命令有误。

## 解决方案

### 正确的启动命令

```bash
# 在 desktop-client 目录中
cd desktop-client
cargo tauri dev
```

### 启动脚本已修复

`scripts/start-all.sh` 已更新为使用正确的命令：

```bash
cd "$PROJECT_ROOT/desktop-client"
cargo tauri dev &
```

---

## 三种启动方式

### 方式 1: 使用启动脚本（推荐）

```bash
export LLM_API_KEY="sk-d162b1775e514757b083a2dfe729d6e6"
bash scripts/start-all.sh
```

这会启动：
- ✅ 后端服务
- ✅ 前端 Web 服务
- ✅ Tauri 客户端

### 方式 2: 单独启动 Tauri

```bash
cd desktop-client
cargo tauri dev
```

### 方式 3: 使用 npm 脚本

如果 desktop-client 有 npm 脚本配置：

```bash
cd desktop-client
npm run tauri dev
```

---

## Tauri 启动流程

1. **编译 Rust 代码** - 编译 Tauri 后端
2. **启动 Web 服务** - 启动前端开发服务器
3. **打开应用窗口** - 显示 Tauri 应用窗口
4. **连接到后端** - 建立与 IronClaw 后端的连接

---

## 启动输出示例

```
✅ 启动 Tauri 客户端...
✅ Tauri 客户端进程 PID: 19235
✅ 等待 Tauri 客户端启动...
✅ Tauri 客户端已启动

Tauri 客户端:
  PID: 19235
  状态: 运行中
```

---

## 故障排除

### Q: Tauri 启动失败

**A**: 检查以下几点：

1. 确保已安装 Rust
   ```bash
   rustc --version
   cargo --version
   ```

2. 确保在 desktop-client 目录中
   ```bash
   cd desktop-client
   ```

3. 查看编译错误
   ```bash
   cargo tauri dev
   ```

### Q: Tauri 窗口没有打开

**A**: 可能是以下原因：

1. 前端开发服务器未启动
2. 后端服务未运行
3. 端口被占用

### Q: Tauri 无法连接到后端

**A**: 检查后端是否运行：

```bash
curl http://localhost:3000/api/health
```

---

## 环境要求

### 必需

- Rust 1.92+
- Node.js 16+
- npm 或 yarn

### 可选

- Xcode（macOS）
- Visual Studio Build Tools（Windows）

---

## 相关文件

- `scripts/start-all.sh` - 启动脚本
- `desktop-client/Cargo.toml` - Tauri 配置
- `desktop-client/tauri.conf.json` - Tauri 应用配置

---

## 快速启动

```bash
# 1. 设置 API 密钥
export LLM_API_KEY="sk-d162b1775e514757b083a2dfe729d6e6"

# 2. 启动所有服务（包括 Tauri）
bash scripts/start-all.sh

# 3. Tauri 应用窗口会自动打开
```

---

## 停止 Tauri

### 方式 1: 关闭应用窗口

直接关闭 Tauri 应用窗口。

### 方式 2: 按 Ctrl+C

在终端中按 `Ctrl+C` 停止所有服务。

### 方式 3: 手动杀死进程

```bash
kill <TAURI_PID>
```

