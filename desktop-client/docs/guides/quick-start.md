# Desktop Client 快速开始

> 5 分钟快速上手 Desktop Client 开发

## 前置要求

- Rust 1.92+
- Node.js 18+
- npm 或 yarn

## 一次性设置

```bash
# 1. 克隆项目
git clone <repository-url>
cd x-claw/desktop-client

# 2. 安装前端依赖
cd src-ui
npm install
cd ..
```

## 启动开发环境

需要**两个终端窗口**:

### 终端 1: 前端开发服务器

```bash
cd desktop-client/src-ui
npm run dev
```

应该看到:
```
VITE v6.3.5  ready in XXX ms
➜  Local:   http://localhost:5173/
```

### 终端 2: Tauri 应用

```bash
cd desktop-client
cargo tauri dev
```

这会:
- ✅ 编译 Rust 后端
- ✅ 打开应用窗口
- ✅ 启用热重载

## 开发工作流

### 修改前端代码

1. 编辑 `src-ui/src/` 中的文件
2. 保存 → 自动刷新 (1-2秒)

### 修改后端代码

1. 编辑 `src/` 中的文件
2. 保存 → 自动重新编译 (5-10秒)

### 查看日志

- 前端日志: 终端 1
- 后端日志: 终端 2
- 浏览器日志: F12 开发者工具

## 常用命令

### 运行测试

```bash
# Rust 测试
cargo test

# 前端测试
cd src-ui
npm run test
```

### 代码检查

```bash
# Rust
cargo clippy
cargo fmt --check

# 前端
cd src-ui
npm run lint
```

### 构建生产版本

```bash
# 1. 构建前端
cd src-ui
npm run build
cd ..

# 2. 构建应用
cargo tauri build
```

输出位置:
- macOS: `target/release/bundle/macos/`
- Linux: `target/release/bundle/deb/`
- Windows: `target/release/bundle/msi/`

## 项目结构

```
desktop-client/
├── src/                    # Rust 后端
│   ├── main.rs            # 应用入口
│   ├── commands.rs        # Tauri 命令
│   └── ...
├── src-ui/                # React 前端
│   ├── src/
│   │   ├── app/
│   │   │   ├── components/  # UI 组件
│   │   │   ├── hooks/       # React Hooks
│   │   │   └── pages/       # 页面组件
│   │   └── main.tsx         # 前端入口
│   └── package.json
├── Cargo.toml             # Rust 依赖
└── tauri.conf.json        # Tauri 配置
```

## 故障排除

### 端口 5173 被占用

```bash
# macOS/Linux
lsof -ti:5173 | xargs kill -9

# Windows
netstat -ano | findstr :5173
taskkill /PID <PID> /F
```

### 编译错误

```bash
# 清除缓存
cargo clean
cd src-ui && rm -rf node_modules dist && npm install && cd ..

# 重新编译
cargo tauri dev
```

### 热重载不工作

1. 确保两个终端都在运行
2. 检查终端输出是否有错误
3. 尝试重启服务

## 下一步

- [开发指南](development.md) - 详细的开发流程
- [架构总览](../architecture/overview.md) - 了解系统架构
- [测试指南](testing.md) - 编写和运行测试

## 常见问题

### Q: 为什么需要两个终端?

A: 前端 (Vite) 和后端 (Tauri) 是独立的进程,需要分别启动。

### Q: 如何调试 Rust 代码?

A: 使用 `tracing::debug!()` 输出日志,或使用 VS Code 的 Rust 调试器。

### Q: 如何调试前端代码?

A: 按 F12 打开开发者工具,使用 Chrome DevTools。

### Q: 如何添加新的 Tauri 命令?

A: 参考 [开发指南](development.md) 中的"添加新命令"章节。

---

**记住**: 开发时需要同时运行 `npm run dev` 和 `cargo tauri dev`!
