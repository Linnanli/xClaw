# IronClaw 服务器架构说明

> 解释 Desktop Client 如何与 IronClaw 服务器交互

## 📋 架构总览

```
┌─────────────────────────────────────────────────────────┐
│  Desktop Client (Tauri 应用)                             │
│  ┌─────────────────────────────────────────────────┐   │
│  │  前端 (React + Vite)                             │   │
│  │  - 端口: 5173                                    │   │
│  │  - UI 组件和用户交互                              │   │
│  └──────────────┬──────────────────────────────────┘   │
│                 │ Tauri IPC                             │
│  ┌──────────────▼──────────────────────────────────┐   │
│  │  Tauri 后端 (Rust)                               │   │
│  │  - Tauri 命令处理                                │   │
│  │  - 认证管理                                      │   │
│  │  - 本地存储                                      │   │
│  └──────────────┬──────────────────────────────────┘   │
└─────────────────┼──────────────────────────────────────┘
                  │
                  │ HTTP/SSE (端口 38080)
                  │
┌─────────────────▼──────────────────────────────────────┐
│  IronClaw 服务器 (外部进程)                              │
│  - 端口: 38080                                          │
│  - AI 聊天功能                                          │
│  - DLP 扫描                                             │
│  - 记忆管理                                             │
│  - 任务管理                                             │
│  - 扩展和技能                                           │
└─────────────────────────────────────────────────────────┘
```

## 🔍 为什么是外部服务器?

### 原来的问题 (已修复)

之前尝试在 Desktop Client 内部启动 IronClaw 服务器:

```rust
// ❌ 不可行的实现
let mut child = tokio::process::Command::new("cargo")
    .args(&["run", "--manifest-path", "../ironclaw/Cargo.toml", ...])
    .spawn()?;
```

**问题**:
1. **生产环境不可行**: 打包后的应用没有 `cargo` 命令
2. **路径依赖**: 需要完整的源代码目录结构
3. **复杂的进程管理**: 需要处理子进程生命周期、日志、错误
4. **调试困难**: 日志混在一起,难以定位问题
5. **资源管理**: 子进程可能成为僵尸进程

### 当前的解决方案

Desktop Client 依赖外部 IronClaw 服务器:

```rust
// ✅ 简单的健康检查
pub async fn check_server_health() -> Result<(), Box<dyn std::error::Error>> {
    let url = format!("http://127.0.0.1:{}/api/health", EMBEDDED_SERVER_PORT);
    let response = client.get(&url).send().await?;
    
    if response.status().is_success() {
        Ok(())
    } else {
        Err("Health check failed".into())
    }
}
```

**优势**:
1. ✅ 架构清晰: Desktop Client 和 IronClaw 服务器职责分离
2. ✅ 易于调试: 两个独立的进程,日志分开
3. ✅ 易于开发: 可以独立重启任一服务
4. ✅ 生产可行: 不依赖开发工具 (cargo)
5. ✅ 资源管理: 操作系统负责进程管理

## 🚀 如何启动 IronClaw 服务器?

### 方式 1: 使用一键启动脚本 (推荐)

```bash
# 在项目根目录
./scripts/start-all.sh
```

**脚本会自动启动**:
1. ✅ 检查 PostgreSQL 数据库 (端口 5432)
2. ✅ 启动 IronClaw 服务器 (端口 38080) ← 这就是你需要的
3. ✅ 启动 Desktop Client 前端 (端口 5173)
4. ✅ 启动 Tauri 客户端
5. ✅ 启动 Admin Backend (端口 3000 + 5174)

**脚本内部的启动命令**:
```bash
# 设置环境变量
export GATEWAY_PORT=38080
export GATEWAY_HOST=127.0.0.1
export GATEWAY_ENABLED=true

# 启动 IronClaw 服务器
cargo run --manifest-path ironclaw/Cargo.toml -- run --no-onboard > /tmp/ironclaw-server.log 2>&1 &
```

### 方式 2: 手动启动 (开发时)

如果你想单独启动服务进行调试:

**终端 1: 启动 IronClaw 服务器**
```bash
# 在项目根目录
export GATEWAY_PORT=38080
export GATEWAY_HOST=127.0.0.1
export GATEWAY_ENABLED=true

cargo run -- run --no-onboard
```

**终端 2: 启动 Desktop Client 前端**
```bash
cd desktop-client/src-ui
npm run dev
```

**终端 3: 启动 Tauri 应用**
```bash
cd desktop-client
cargo tauri dev
```

### 方式 3: 仅启动 IronClaw 服务器

如果你只想测试 IronClaw 服务器:

```bash
# 在项目根目录
cargo run -- run --no-onboard
```

然后访问: http://localhost:38080/api/health

## 🔄 启动流程详解

### Desktop Client 启动时

1. **加载配置** (main.rs)
   ```rust
   // 加载环境变量和配置
   let config = Config::builder()
       .add_source(File::with_name(".env"))
       .add_source(Environment::default())
       .build()?;
   ```

2. **检查 IronClaw 服务器** (embedded_server.rs)
   ```rust
   // 检查外部服务器是否运行
   match check_server_health().await {
       Ok(()) => {
           println!("✅ IronClaw server is running");
       }
       Err(_) => {
           print_server_instructions();
           // 应用继续运行,但聊天功能不可用
       }
   }
   ```

3. **启动 Tauri 应用**
   ```rust
   tauri::Builder::default()
       .manage(state)
       .invoke_handler(tauri::generate_handler![
           send_chat_message,
           subscribe_chat_events,
           // ... 其他命令
       ])
       .run(tauri::generate_context!())
   ```

### IronClaw 服务器启动时

1. **初始化数据库连接**
2. **加载配置和扩展**
3. **启动 Web Gateway** (端口 38080)
4. **注册 API 路由**:
   - `/api/health` - 健康检查
   - `/api/chat/send` - 发送消息
   - `/api/chat/events` - SSE 事件流
   - `/api/dlp/scan` - DLP 扫描
   - 等等...

## 📡 通信方式

### HTTP 请求

Desktop Client → IronClaw 服务器

```rust
// 发送聊天消息
let response = client
    .post("http://localhost:38080/api/chat/send")
    .json(&request)
    .send()
    .await?;
```

### SSE 事件流

IronClaw 服务器 → Desktop Client

```rust
// 订阅聊天事件
let event_source = EventSource::new("http://localhost:38080/api/chat/events")?;

event_source.addEventListener("response", |event| {
    // 处理 AI 回复
});

event_source.addEventListener("thinking", |event| {
    // 处理思考状态
});
```

## 🛠️ 开发工作流

### 修改 Desktop Client 代码

1. 编辑 `desktop-client/src/` 或 `desktop-client/src-ui/src/`
2. 保存 → 自动重新编译/刷新
3. 无需重启 IronClaw 服务器

### 修改 IronClaw 服务器代码

1. 编辑 `src/` (主项目)
2. 重启 IronClaw 服务器:
   ```bash
   # 按 Ctrl+C 停止
   # 重新运行
   cargo run -- run --no-onboard
   ```
3. Desktop Client 会自动重连

### 查看日志

**IronClaw 服务器日志**:
```bash
# 如果使用 start-all.sh
tail -f /tmp/ironclaw-server.log

# 如果手动启动
# 直接在终端查看
```

**Desktop Client 日志**:
```bash
# Tauri 后端日志
tail -f /tmp/tauri.log

# 前端日志
tail -f /tmp/desktop-frontend.log

# 或在浏览器开发者工具查看
```

## 🔍 故障排查

### IronClaw 服务器未启动

**症状**: Desktop Client 显示 "External IronClaw server is not running"

**解决**:
```bash
# 检查服务器是否运行
curl http://localhost:38080/api/health

# 如果没有响应,启动服务器
cargo run -- run --no-onboard

# 或使用一键启动脚本
./scripts/start-all.sh
```

### 端口被占用

**症状**: "Address already in use (os error 48)"

**解决**:
```bash
# 查找占用端口的进程
lsof -i :38080

# 杀死进程
kill -9 <PID>

# 或使用脚本自动清理
./scripts/start-all.sh  # 脚本会自动清理端口
```

### 连接超时

**症状**: "Connection timeout" 或 "Connection refused"

**原因**:
1. IronClaw 服务器未启动
2. 防火墙阻止连接
3. 端口配置错误

**解决**:
```bash
# 1. 确认服务器运行
curl http://localhost:38080/api/health

# 2. 检查端口配置
echo $GATEWAY_PORT  # 应该是 38080

# 3. 检查防火墙设置 (macOS)
# 系统偏好设置 → 安全性与隐私 → 防火墙
```

## 📚 相关文档

- [CODE_OPTIMIZATION_PLAN.md](./CODE_OPTIMIZATION_PLAN.md) - Task 2.1 详细说明
- [docs/architecture/overview.md](./docs/architecture/overview.md) - 架构总览
- [docs/guides/quick-start.md](./docs/guides/quick-start.md) - 快速开始
- [../QUICK_START.md](../QUICK_START.md) - 项目快速启动指南
- [../scripts/start-all.sh](../scripts/start-all.sh) - 一键启动脚本

## 💡 常见问题

### Q: 为什么不内嵌 IronClaw 服务器?

A: 内嵌服务器需要在 Desktop Client 内部启动 cargo 子进程,这在生产环境不可行 (打包后的应用没有 cargo 命令)。外部服务器架构更清晰、更易维护。

### Q: 生产环境如何部署?

A: 生产环境会将 IronClaw 服务器打包为独立的可执行文件,Desktop Client 连接到该服务器。具体部署方案待定。

### Q: 可以同时运行多个 Desktop Client 吗?

A: 可以。多个 Desktop Client 可以连接到同一个 IronClaw 服务器 (端口 38080)。

### Q: IronClaw 服务器可以远程访问吗?

A: 当前配置为本地访问 (127.0.0.1)。如需远程访问,需要修改 `GATEWAY_HOST` 环境变量并配置防火墙。

### Q: 如何切换到不同的 IronClaw 服务器?

A: 修改 `desktop-client/src/embedded_server.rs` 中的 `EMBEDDED_SERVER_PORT` 常量,或通过环境变量配置。

---

**总结**: Desktop Client 依赖外部 IronClaw 服务器,通过 HTTP/SSE 通信。使用 `./scripts/start-all.sh` 一键启动所有服务,或手动启动进行调试。
