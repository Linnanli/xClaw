# 后端启动问题修复

## 问题描述

运行 `scripts/start-all.sh` 时，后端启动失败，报错：

```
error: `cargo run` could not determine which binary to run. Use the `--bin` option to specify a binary, or the `default-run` manifest key.
available binaries: admin-backend, desktop-client
```

## 根本原因

项目采用了 workspace + git submodule 的架构：

```
x-claw/                    # 项目根目录
├── Cargo.toml            # Workspace 配置（包含 desktop-client 和 admin-backend）
├── ironclaw/             # Git 子模块（主项目）
│   ├── Cargo.toml        # 主项目配置（包含 ironclaw 二进制）
│   └── src/              # 主项目源代码
├── desktop-client/       # Desktop Client 应用
└── admin-backend/        # Admin Backend 应用
```

**问题**：
- 启动脚本在项目根目录运行 `cargo run`
- 根目录的 `Cargo.toml` 只定义了 workspace，没有可执行的二进制文件
- 主项目 `ironclaw` 在子模块中，需要进入子模块目录才能运行

## 解决方案

### 1. 修改 `scripts/start-all.sh`

**修改前**：
```bash
(exec cargo run -- run --no-onboard < /dev/null > /tmp/backend.log 2>&1) &
```

**修改后**：
```bash
(cd ironclaw && exec cargo run -- run --no-onboard < /dev/null > /tmp/backend.log 2>&1) &
```

**关键变化**：
- 添加 `cd ironclaw` 进入子模块目录
- 在子模块目录中运行 `cargo run`

### 2. 修改清理进程的命令

**修改前**：
```bash
pkill -f "cargo run" || true
```

**修改后**：
```bash
pkill -f "ironclaw.*run" || true
```

**原因**：
- 更精确地匹配 ironclaw 进程
- 避免误杀其他 cargo run 进程

### 3. 创建简化的后端启动脚本

创建了 `scripts/start-backend-only.sh`，只启动后端服务，方便测试和调试。

## 使用方法

### 启动完整服务（后端 + 前端 + Tauri）

```bash
./scripts/start-all.sh
```

### 仅启动后端服务

```bash
./scripts/start-backend-only.sh
```

### 查看后端日志

```bash
tail -f /tmp/backend.log
```

### 检查后端健康状态

```bash
curl http://localhost:3000/api/health
```

## 注意事项

### 首次编译

首次运行时，Rust 需要编译所有依赖，可能需要 5-10 分钟：

```
编译进度: .......................
```

请耐心等待，编译完成后会显示：

```
✅ 后端编译完成并启动成功
```

### 后续启动

编译完成后，后续启动会快很多（通常 10-30 秒）。

### 环境变量

确保设置了以下环境变量之一：

- `ANTHROPIC_API_KEY` - Claude API 密钥
- `OPENAI_API_KEY` - OpenAI API 密钥
- `LLM_API_KEY` - 兼容 OpenAI 的 API 密钥（如 Qwen）
- `NEARAI_API_KEY` - NEAR AI API 密钥

可以在以下位置配置：

1. 项目根目录的 `.env` 文件（优先级最高）
2. `~/.ironclaw/.env` 文件（后备配置）

### 端口占用

脚本会自动清理以下端口：

- `3000` - 后端服务
- `5173` - 前端开发服务器
- `8080` - Tauri 客户端

如果端口被占用，脚本会自动杀死占用进程。

## 架构说明

### 项目结构

```
x-claw/
├── ironclaw/              # 主项目（Git 子模块）
│   ├── src/              # 核心功能实现
│   ├── channels/         # 通信通道（Web、CLI 等）
│   └── tools/            # 工具和扩展
├── desktop-client/        # Desktop Client（Tauri 应用）
│   ├── src-tauri/        # Rust 后端
│   └── src-ui/           # React 前端
└── admin-backend/         # Admin Backend（管理后台）
    └── src/              # Rust 后端
```

### 依赖关系

```
Desktop Client → Web Gateway API → ironclaw 核心功能
Admin Backend → ironclaw crate → ironclaw 核心功能
```

### 启动顺序

1. 后端服务（ironclaw）- 端口 3000
2. 前端开发服务器（Vite）- 端口 5173
3. Tauri 客户端（连接到前端开发服务器）

## 故障排查

### 后端启动失败

1. 查看后端日志：
   ```bash
   tail -100 /tmp/backend.log
   ```

2. 检查编译错误：
   ```bash
   cd ironclaw
   cargo build
   ```

3. 检查环境变量：
   ```bash
   echo $LLM_API_KEY
   echo $ANTHROPIC_API_KEY
   ```

### 端口被占用

1. 查看端口占用：
   ```bash
   lsof -i :3000
   lsof -i :5173
   ```

2. 手动清理端口：
   ```bash
   lsof -i :3000 | grep -v COMMAND | awk '{print $2}' | xargs kill -9
   ```

### 前端连接失败

1. 确认后端已启动：
   ```bash
   curl http://localhost:3000/api/health
   ```

2. 检查认证令牌：
   ```bash
   grep "gateway.*http" /tmp/backend.log | tail -1
   ```

3. 使用带令牌的 URL 访问前端：
   ```
   http://localhost:5173?token=<TOKEN>
   ```

## 相关文档

- `scripts/start-all.sh` - 完整启动脚本
- `scripts/start-backend-only.sh` - 仅启动后端
- `AGENTS.md` - Agent 规则和测试策略
- `README.md` - 项目说明
