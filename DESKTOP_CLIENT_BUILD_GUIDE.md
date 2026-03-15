# IronClaw Desktop Client - 编译和启动指南

本文档说明如何编译和启动 IronClaw 桌面客户端（基于 Tauri）。

## 系统要求

### 必需工具

- **Rust 1.92+** - 编程语言和工具链
  - 安装: https://rustup.rs/
  - 验证: `rustc --version`

- **Node.js 18+** (可选，仅用于前端开发)
  - 安装: https://nodejs.org/
  - 验证: `node --version`

### 平台特定要求

#### macOS
- Xcode Command Line Tools
  ```bash
  xcode-select --install
  ```
- 或完整的 Xcode (从 App Store)

#### Linux (Ubuntu/Debian)
```bash
sudo apt-get install libwebkit2gtk-4.1-dev \
  build-essential \
  curl \
  wget \
  file \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev
```

#### Windows
- Visual Studio Build Tools 2022 或 Visual Studio Community
- 或 MinGW-w64 工具链

## 项目结构

```
desktop-client/
├── src/                    # Rust 后端代码
│   ├── lib.rs             # 库入口
│   ├── main.rs            # 应用入口
│   ├── commands.rs        # Tauri 命令
│   ├── auth.rs            # 认证管理
│   ├── storage.rs         # 本地存储
│   ├── extension_manager.rs    # 扩展管理
│   ├── routine_manager.rs      # 日程管理
│   └── ...
├── src-ui/                # 前端代码
│   ├── index.html         # HTML 模板
│   ├── app-desktop.js     # JavaScript 逻辑
│   └── style.css          # 样式表
├── tests/                 # 测试文件
│   ├── extension_manager_property_tests.rs
│   ├── routine_manager_property_tests.rs
│   └── ...
├── Cargo.toml            # Rust 依赖配置
├── tauri.conf.json       # Tauri 应用配置
└── build.rs              # 构建脚本
```

## 编译步骤

### 1. 克隆或进入项目目录

```bash
cd /path/to/ironclaw
```

### 2. 检查 Rust 工具链

```bash
rustc --version
cargo --version
```

预期输出:
```
rustc 1.92.0 (or higher)
cargo 1.92.0 (or higher)
```

### 3. 编译 Rust 后端

#### 开发模式编译（快速，包含调试信息）

```bash
cargo build -p desktop-client
```

预期输出:
```
   Compiling desktop-client v0.1.0
    Finished dev [unoptimized + debuginfo] target(s) in XX.XXs
```

#### 发布模式编译（优化，文件较大）

```bash
cargo build -p desktop-client --release
```

预期输出:
```
   Compiling desktop-client v0.1.0
    Finished release [optimized] target(s) in XX.XXs
```

### 4. 启动应用

有三种方式启动 desktop-client：

#### 方式 1: 使用 cargo run（最简单，推荐用于开发）

```bash
cargo run -p desktop-client
```

这会:
- 自动编译（如果代码有修改）
- 启动应用窗口
- 显示控制台日志

**首次启动时间**: 约 20-30 秒（需要编译）
**后续启动时间**: 约 5-10 秒（增量编译）

#### 方式 2: 直接运行编译好的二进制文件（最快）

开发模式（需要先运行 `cargo build -p desktop-client`）:
```bash
./target/debug/desktop-client
```

发布模式（需要先运行 `cargo build -p desktop-client --release`）:
```bash
./target/release/desktop-client
```

**启动时间**: 1-2 秒

#### 方式 3: 使用 Tauri CLI（用于前端开发，支持热重载）

首先安装 Tauri CLI（仅需一次）:
```bash
cargo install tauri-cli
```

然后运行开发服务器:
```bash
cd desktop-client
cargo tauri dev
```

这会:
- 启动 Rust 后端
- 监听文件变化
- 打开应用窗口
- 启用热重载（修改前端代码后自动刷新）

**注意**: 此方式主要用于前端开发，修改 Rust 代码仍需重新编译。

### 启动后的界面

应用启动后会显示:

1. **认证屏幕**（首次使用）
   - 输入主密码（至少12个字符，包含大小写字母和数字）
   - 点击 "Setup Master Password" 按钮

2. **主界面**（认证后）
   - 5个标签页: Chat, Approvals, Plugins, Extensions, Routines
   - 左侧导航栏
   - 右上角 Lock 按钮

### 数据存储位置

应用数据存储在:
- **macOS**: `~/Library/Application Support/ironclaw/`
- **Linux**: `~/.local/share/ironclaw/`
- **Windows**: `%APPDATA%\ironclaw\`

包含:
- `desktop.db` - 本地数据库（加密）
- 配置文件
- 审计日志

## 验证编译

### 检查编译错误

```bash
cargo check -p desktop-client
```

预期输出:
```
    Checking desktop-client v0.1.0
    Finished check [unoptimized + debuginfo] target(s) in XX.XXs
```

### 运行单元测试

```bash
cargo test -p desktop-client --lib
```

预期输出:
```
running X tests

test result: ok. X passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### 运行属性测试

```bash
cargo test -p desktop-client --test extension_manager_property_tests
cargo test -p desktop-client --test routine_manager_property_tests
```

## 应用功能验证

启动应用后（使用 `cargo run -p desktop-client`），验证以下功能:

### 1. 首次启动 - 主密码设置
- [ ] 显示 "IronClaw Desktop" 窗口标题
- [ ] 显示 "Setup Master Password" 输入框
- [ ] 输入密码（至少12字符，包含大小写字母和数字）
- [ ] 点击 "Setup" 按钮成功设置密码

### 2. 认证屏幕（后续启动）
- [ ] 显示 "Unlock IronClaw" 标题
- [ ] 主密码输入框可用
- [ ] "Unlock" 按钮可点击
- [ ] 输入正确密码后进入主界面

### 3. 主界面标签页
- [ ] **Chat** 标签页 - 对话界面
- [ ] **Approvals** 标签页 - 操作审批
- [ ] **Plugins** 标签页 - 插件管理
- [ ] **Extensions** 标签页 - 扩展管理（新增）
- [ ] **Routines** 标签页 - 日程管理（新增）

### 4. Extensions 标签页功能（新增）
- [ ] 显示 "Extensions Management" 标题
- [ ] 搜索框可用（输入关键词搜索扩展）
- [ ] "Refresh" 按钮可用
- [ ] "Installed" 和 "Available" 两个子标签页
- [ ] 扩展卡片显示:
  - 扩展名称和版本
  - 作者信息
  - 描述文本
  - 提供的工具列表
  - 权限列表
- [ ] 操作按钮:
  - Install（安装）
  - Uninstall（卸载）
  - Enable（启用）
  - Disable（禁用）

### 5. Routines 标签页功能（新增）
- [ ] 显示 "Routines Management" 标题
- [ ] "+ New Routine" 按钮可用
- [ ] 日程列表显示（如果已创建日程）
- [ ] 日程卡片显示:
  - 日程名称
  - 描述
  - 触发器类型（Time/Event/Manual）
  - 状态徽章（Active/Paused/Disabled）
- [ ] 操作按钮:
  - Trigger（手动触发）
  - Enable（启用）
  - Disable（禁用）
  - Delete（删除）
- [ ] 创建日程模态框:
  - 名称输入框
  - 描述输入框
  - 触发器类型选择（Time/Event/Manual）
  - 触发器值输入（如 cron 表达式）
  - Create 和 Cancel 按钮

### 6. 基本操作测试
- [ ] 切换标签页正常
- [ ] 点击 "Lock" 按钮返回认证屏幕
- [ ] 窗口可以调整大小（最小 800x600）
- [ ] 窗口可以最小化/最大化/关闭

## 常见问题

### Tauri API 不可用 (window.__TAURI_INTERNALS__ undefined)

**问题描述:**
应用启动后，JavaScript 控制台显示 `window.__TAURI_INTERNALS__ is undefined`，导致所有 Tauri 命令调用失败。

**原因:**
Tauri 2.0 改变了 API 注入机制。不再使用 `window.__TAURI__`，而是使用 `window.__TAURI_INTERNALS__` 进行 IPC 通信。

**解决方案:**

1. **必须使用 `cargo tauri dev` 启动**
   ```bash
   cargo tauri dev
   ```
   不要使用 `cargo run`，因为它不会注入 Tauri API。

2. **验证 Tauri API 可用性**
   在浏览器开发者工具（F12）中运行:
   ```javascript
   console.log(typeof window.__TAURI_INTERNALS__);
   // 应该输出: "object"
   ```

3. **清除缓存并重新编译**
   ```bash
   cargo clean
   cargo tauri dev
   ```

4. **检查 Tauri CLI 版本**
   确保 Tauri CLI 已安装且版本匹配:
   ```bash
   cargo install tauri-cli --version "^2.0"
   cargo tauri --version
   ```

### 编译错误: "cannot find crate `tauri`"

**解决方案:**
```bash
cargo update
cargo build -p desktop-client
```

### 编译错误: "libwebkit2gtk-4.1 not found" (Linux)

**解决方案:**
```bash
sudo apt-get install libwebkit2gtk-4.1-dev
```

### 应用启动后立即崩溃

**检查步骤:**
1. 查看控制台输出中的错误信息
2. 检查 `~/.ironclaw/` 目录是否存在
3. 尝试删除缓存: `rm -rf ~/.ironclaw/`
4. 重新启动应用

### 前端不显示

**解决方案:**
1. 检查 `src-ui/` 目录中的文件是否存在
2. 验证 `tauri.conf.json` 中的 `frontendDist` 路径正确
3. 查看浏览器开发者工具（F12）中的错误

## 开发工作流

### 修改 Rust 代码

1. 编辑 `src/` 中的文件
2. 如果使用 `cargo tauri dev`，应用会自动重新编译
3. 否则手动运行 `cargo build -p desktop-client`

### 修改前端代码

1. 编辑 `src-ui/` 中的文件（HTML/CSS/JS）
2. 如果使用 `cargo tauri dev`，应用会自动重新加载
3. 否则手动刷新应用窗口（Cmd+R 或 Ctrl+R）

### 添加新的 Tauri 命令

1. 在 `src/commands.rs` 中添加新函数，使用 `#[tauri::command]` 属性
2. 在 `src-ui/app-desktop.js` 中调用: `window.__TAURI__.invoke('command_name', { args })`
3. 重新编译应用

## 性能优化

### 开发模式 vs 发布模式

| 方面 | 开发模式 | 发布模式 |
|------|--------|--------|
| 编译时间 | 快 (1-2 分钟) | 慢 (5-10 分钟) |
| 二进制大小 | 大 (100+ MB) | 小 (30-50 MB) |
| 运行速度 | 慢 | 快 |
| 调试信息 | 包含 | 不包含 |
| 用途 | 开发/测试 | 生产发布 |

### 增量编译

Cargo 会自动缓存编译结果。只有修改的文件会被重新编译。

清除缓存:
```bash
cargo clean
```

## 部署

### 创建可分发的应用

#### macOS
```bash
cargo tauri build -p desktop-client
# 输出: target/release/bundle/macos/Ironclaw\ Desktop.app
```

#### Linux
```bash
cargo tauri build -p desktop-client
# 输出: target/release/bundle/deb/ironclaw-desktop_*.deb
```

#### Windows
```bash
cargo tauri build -p desktop-client
# 输出: target/release/bundle/msi/Ironclaw\ Desktop_*.msi
```

## 调试

### 启用详细日志

```bash
RUST_LOG=debug cargo tauri dev -p desktop-client
```

### 使用浏览器开发者工具

在应用运行时按 F12 打开开发者工具:
- Console: 查看 JavaScript 错误和日志
- Network: 查看 Tauri 命令调用
- Elements: 检查 HTML 结构

### 查看 Rust 日志

日志会输出到控制台。使用 `tracing` 宏:
```rust
tracing::info!("Message");
tracing::debug!("Debug info");
tracing::error!("Error message");
```

## 相关文档

- [Tauri 官方文档](https://tauri.app/)
- [Rust 官方文档](https://doc.rust-lang.org/)
- [项目架构文档](.kiro/specs/enterprise-ai-agent-platform/ARCHITECTURE_CORRECTED.md)
- [任务跟踪](.kiro/specs/enterprise-ai-agent-platform/tasks.md)

## 获取帮助

如遇到问题:

1. 检查本文档的"常见问题"部分
2. 查看编译错误信息（通常包含解决方案）
3. 检查 Tauri 官方文档
4. 查看项目的 GitHub Issues

---

**最后更新**: 2026-03-15
**文档版本**: 1.0
