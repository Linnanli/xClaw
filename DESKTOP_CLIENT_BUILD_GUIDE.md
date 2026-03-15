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
├── src-ui-new/            # 新的 React 前端（推荐）
│   ├── src/
│   │   ├── app/
│   │   │   ├── components/    # React 组件
│   │   │   ├── contexts/      # React Context
│   │   │   ├── utils/         # 工具函数（含 Tauri API）
│   │   │   ├── App.tsx
│   │   │   └── routes.tsx
│   │   ├── styles/            # 样式文件
│   │   └── main.tsx           # 入口文件
│   ├── package.json
│   ├── vite.config.ts
│   └── tsconfig.json
├── src-ui-backup/         # 原有前端备份
│   ├── index.html
│   ├── app-desktop.js
│   └── style.css
├── tests/                 # 测试文件
│   ├── extension_manager_property_tests.rs
│   ├── routine_manager_property_tests.rs
│   └── ...
├── Cargo.toml            # Rust 依赖配置
├── tauri.conf.json       # Tauri 应用配置
├── build.rs              # 构建脚本
└── UI_MIGRATION_GUIDE.md # UI 迁移指南
```

## 前端技术栈

项目已从原生 HTML/JavaScript 迁移到现代化的 React 技术栈：

- **框架**: React 18 + TypeScript
- **构建工具**: Vite 6
- **UI 组件**: Radix UI + Tailwind CSS
- **路由**: React Router 7
- **主题**: 支持深色/浅色模式切换

详细迁移信息请参考 `UI_MIGRATION_GUIDE.md`。

## 编译步骤

### 1. 克隆或进入项目目录

```bash
cd /path/to/ironclaw
```

### 2. 安装前端依赖

项目使用 React + Vite 构建前端，需要先安装 npm 依赖：

```bash
cd desktop-client/src-ui-new
npm install
cd ..
```

预期输出:
```
added 284 packages in XXs
```

**注意**: 这一步只需要执行一次，除非 `package.json` 发生变化。

### 3. 检查 Rust 工具链

```bash
rustc --version
cargo --version
```

预期输出:
```
rustc 1.92.0 (or higher)
cargo 1.92.0 (or higher)
```

### 4. 编译 Rust 后端

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

### 5. 启动应用

**重要**: 由于前端使用 Vite 构建，必须使用 `cargo tauri dev` 或 `cargo tauri build` 命令。

#### 方式 1: 使用 Tauri CLI 开发模式（推荐）

**需要两个终端窗口：**

**终端 1 - 启动 Vite 开发服务器：**
```bash
cd desktop-client/src-ui-new
npm run dev
```

**终端 2 - 启动 Tauri 应用：**
```bash
cd desktop-client
cargo tauri dev
```

**这会自动完成以下操作：**
1. Vite 启动开发服务器 (http://localhost:5173)
2. Rust 后端编译
3. 应用窗口打开
4. **支持热重载**：
   - 修改 React/TypeScript 代码 → 浏览器自动刷新（1-2秒）
   - 修改 Rust 代码 → 自动重新编译并重启应用（5-10秒）

**首次启动时间**: 约 30-60 秒（需要编译 Rust 和启动 Vite）
**后续启动时间**: 约 10-15 秒（增量编译）

**开发体验：**
- ✅ 修改 React 组件 → 立即看到效果（无需手动刷新）
- ✅ 修改样式 → 立即更新
- ✅ TypeScript 类型检查 → 实时提示
- ✅ 控制台日志 → 同时显示前端和后端日志

**注意事项：**
- 需要保持两个终端窗口打开
- 如果端口 5173 被占用，修改 `vite.config.ts` 中的端口
- 关闭任一终端会停止相应的服务

#### 方式 2: 构建生产版本

```bash
cd desktop-client
cargo tauri build
```

这会:
- 构建前端生产版本 (`npm run build`)
- 编译 Rust 后端（发布模式）
- 创建可分发的应用包

**构建时间**: 约 5-10 分钟

输出位置:
- **macOS**: `target/release/bundle/macos/Ironclaw Desktop.app`
- **Linux**: `target/release/bundle/deb/ironclaw-desktop_*.deb`
- **Windows**: `target/release/bundle/msi/Ironclaw Desktop_*.msi`

#### ~~方式 3: 直接运行二进制文件（不推荐）~~

**注意**: 由于前端需要 Vite 构建，不建议直接运行 `cargo run` 或二进制文件。
如果必须这样做，需要先手动构建前端：

```bash
cd desktop-client/src-ui-new
npm run build
cd ..
cargo run -p desktop-client
```

### 启动后的界面

应用启动后会显示现代化的 React UI：

1. **认证屏幕**（首次使用）
   - 精美的渐变背景和动画
   - 主密码设置界面
   - 密码强度指示器
   - 实时验证提示

2. **主界面**（认证后）
   - 顶部标签栏: 聊天、记忆、任务、日程、扩展、技能
   - 右侧: 日志按钮、连接状态、主题切换
   - 响应式布局，支持深色/浅色模式
   - 流畅的动画和过渡效果

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
   cd desktop-client
   cargo tauri dev
   ```
   不要使用 `cargo run` 或 `npm run dev`，因为它们不会注入 Tauri API。

2. **验证 Tauri API 可用性**
   在浏览器开发者工具（F12）中运行:
   ```javascript
   console.log(typeof window.__TAURI_INTERNALS__);
   // 应该输出: "object"
   ```

3. **清除缓存并重新编译**
   ```bash
   cargo clean
   cd src-ui-new && npm run build && cd ..
   cargo tauri dev
   ```

4. **检查 Tauri CLI 版本**
   确保 Tauri CLI 已安装且版本匹配:
   ```bash
   cargo install tauri-cli --version "^2.0"
   cargo tauri --version
   ```

### 前端构建失败

**问题描述:**
运行 `cargo tauri dev` 时，Vite 构建失败。

**解决方案:**

1. **检查 Node.js 版本**
   ```bash
   node --version
   # 应该是 18.0.0 或更高
   ```

2. **重新安装依赖**
   ```bash
   cd desktop-client/src-ui-new
   rm -rf node_modules package-lock.json
   npm install
   cd ..
   ```

3. **检查 TypeScript 错误**
   ```bash
   cd desktop-client/src-ui-new
   npm run build
   # 查看详细的错误信息
   ```

### Vite 开发服务器端口被占用

**问题描述:**
`cargo tauri dev` 启动失败，提示端口 5173 已被占用。

**解决方案:**

1. **杀死占用端口的进程**
   ```bash
   # macOS/Linux
   lsof -ti:5173 | xargs kill -9
   
   # Windows
   netstat -ano | findstr :5173
   taskkill /PID <PID> /F
   ```

2. **或修改 Vite 端口**
   编辑 `desktop-client/src-ui-new/vite.config.ts`:
   ```typescript
   export default defineConfig({
     server: {
       port: 5174, // 改为其他端口
     },
   });
   ```
   
   同时更新 `desktop-client/tauri.conf.json`:
   ```json
   {
     "build": {
       "devUrl": "http://localhost:5174"
     }
   }
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
1. 检查 `src-ui-new/dist/` 目录是否存在（生产构建后）
2. 验证 `tauri.conf.json` 中的 `frontendDist` 路径: `"./src-ui-new/dist"`
3. 确保使用 `cargo tauri dev` 而不是 `cargo run`
4. 查看浏览器开发者工具（F12）中的错误
5. 检查 Vite 开发服务器是否正常启动（查看控制台输出）

## 开发工作流

### 修改 Rust 代码

1. 编辑 `src/` 中的文件
2. 如果使用 `cargo tauri dev`，应用会自动重新编译
3. 查看控制台输出中的编译结果

### 修改前端代码（React/TypeScript）

**开发模式下（推荐）：**

1. 启动开发服务器（只需一次）:
   ```bash
   cd desktop-client
   cargo tauri dev
   ```

2. 编辑 `src-ui-new/src/` 中的文件
   - 修改 React 组件 → 浏览器自动刷新（1-2秒）
   - 修改样式 → 立即更新
   - 添加新文件 → 自动识别并热重载

3. 查看效果
   - 无需手动刷新
   - 无需重新运行命令
   - 保持 `cargo tauri dev` 运行即可

**生产构建：**

只有在准备发布时才需要手动构建：
```bash
cd desktop-client/src-ui-new
npm run build
```

**常见问题：**

Q: 修改代码后没有自动刷新？
A: 检查：
- `cargo tauri dev` 是否正在运行
- 浏览器开发者工具（F12）中是否有错误
- Vite 开发服务器是否正常（查看终端输出）

Q: 热重载太慢？
A: 这是正常的，Vite 热重载通常在 1-2 秒内完成。如果超过 5 秒，可能是：
- TypeScript 类型检查耗时（可以暂时禁用）
- 文件太大（考虑拆分组件）
- 电脑性能问题

### 添加新的 React 组件

1. 在 `src-ui-new/src/app/components/` 中创建新组件
2. 使用 TypeScript 编写，确保类型安全
3. 导入并使用 Tauri API: `import { authApi } from '@/app/utils/tauri'`
4. Vite 会自动热重载

### 添加新的 Tauri 命令

1. 在 `src/commands.rs` 中添加新函数，使用 `#[tauri::command]` 属性
2. 在 `src-ui-new/src/app/utils/tauri.ts` 中添加类型定义和 API 函数
3. 在 React 组件中调用: `await authApi.unlockApp(password)`
4. 重新编译应用

### 前端构建流程

开发模式:
```bash
cd desktop-client
cargo tauri dev
# Vite 会自动启动开发服务器
```

生产构建:
```bash
cd desktop-client/src-ui-new
npm run build
# 输出到 dist/ 目录
```

手动测试前端（不启动 Tauri）:
```bash
cd desktop-client/src-ui-new
npm run dev
# 访问 http://localhost:5173
# 注意: Tauri API 不可用
```

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

- [UI 迁移指南](desktop-client/UI_MIGRATION_GUIDE.md) - React UI 迁移详细说明
- [Tauri 官方文档](https://tauri.app/)
- [Vite 官方文档](https://vitejs.dev/)
- [React 官方文档](https://react.dev/)
- [Rust 官方文档](https://doc.rust-lang.org/)
- [项目架构文档](.kiro/specs/enterprise-ai-agent-platform/ARCHITECTURE_CORRECTED.md)
- [任务跟踪](.kiro/specs/enterprise-ai-agent-platform/tasks.md)

## 获取帮助

如遇到问题:

1. 检查本文档的"常见问题"部分
2. 查看 `UI_MIGRATION_GUIDE.md` 了解前端架构
3. 查看编译错误信息（通常包含解决方案）
4. 检查 Tauri 和 Vite 官方文档
5. 查看项目的 GitHub Issues

---

**最后更新**: 2026-03-15
**文档版本**: 2.0 (React UI)
