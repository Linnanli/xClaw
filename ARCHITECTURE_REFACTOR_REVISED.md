# 架构重构修订方案（基于现有 Submodule）

## 当前状态分析

✅ **已完成**:
- IronClaw 已作为 git submodule 添加到 `ironclaw/` 目录
- Submodule 指向: https://github.com/nearai/ironclaw.git
- 当前版本: main 分支（最新版本）

⚠️ **需要调整**:
- 当前仓库结构混合了我们的代码和 IronClaw submodule
- 需要重新组织目录结构
- 需要更新 Cargo.toml 工作空间配置

## 目标架构

```
x-claw/ (当前仓库，重命名为 ironclaw-suite)
├── ironclaw/                    # IronClaw submodule（已存在）
│   ├── src/                     # 核心代码（不修改）
│   ├── Cargo.toml              # 原始配置
│   └── ...
├── desktop-client/             # 桌面客户端（已存在）
│   ├── src/
│   ├── src-ui/
│   └── Cargo.toml
├── admin-backend/              # 管理后台（已存在）
│   ├── src/
│   └── Cargo.toml
├── crates/                     # 共享 Crate（已存在）
│   └── ironclaw_auth/         # 认证模块（我们创建的）
├── ironclaw/crates/           # IronClaw 的 Crate
│   └── ironclaw_safety/       # 安全和 DLP 模块（IronClaw 原生）
├── Cargo.toml                  # 需要更新：工作空间配置
├── README.md                   # 需要更新
└── docs/                       # 文档目录
    ├── ARCHITECTURE.md
    └── MIGRATION_GUIDE.md
```

## 关键差异

### 与原方案的不同

| 项目 | 原方案B | 修订方案 |
|------|---------|----------|
| IronClaw 位置 | 需要克隆 | ✅ 已作为 submodule 存在 |
| 仓库创建 | 创建新仓库 | ✅ 使用当前仓库 |
| 目录迁移 | 需要复制 | ✅ 已在正确位置 |
| 主要工作 | 迁移和配置 | 仅需配置调整 |

## 执行步骤（简化版）

### 阶段 1: 清理和准备（30分钟）

#### 1.1 移除我们对 IronClaw 核心的修改

```bash
# 检查当前 Cargo.toml
cat Cargo.toml

# 备份当前配置
cp Cargo.toml Cargo.toml.backup
```

#### 1.2 确认 IronClaw submodule 版本

```bash
# 切换到稳定版本（推荐 v0.18.0）
cd ironclaw
git checkout v0.18.0
cd ..

# 或者使用最新的 staging 分支
cd ironclaw
git checkout staging
cd ..
```

### 阶段 2: 更新工作空间配置（1小时）

#### 2.1 创建新的根 Cargo.toml

当前的 `Cargo.toml` 包含了 IronClaw 的配置，需要改为纯工作空间配置：

```toml
[workspace]
members = [
    "desktop-client",
    "admin-backend",
    "crates/ironclaw_auth",
]

# IronClaw 作为独立的工作空间，通过路径依赖使用
# ironclaw_safety 使用 IronClaw 中的版本

resolver = "2"

[workspace.package]
edition = "2024"
rust-version = "1.92"
authors = ["NEAR AI <support@near.ai>"]
license = "MIT OR Apache-2.0"

[workspace.dependencies]
# 共享依赖版本
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
thiserror = "2"
tracing = "0.1"

# IronClaw 核心（作为路径依赖）
ironclaw = { path = "ironclaw" }

# 共享 Crate
ironclaw_auth = { path = "crates/ironclaw_auth" }
ironclaw_safety = { path = "ironclaw/crates/ironclaw_safety" }  # 使用 IronClaw 中的版本
```

#### 2.2 更新 Desktop Client 的 Cargo.toml

```toml
[dependencies]
# 使用工作空间依赖
ironclaw = { workspace = true }
ironclaw_auth = { workspace = true }
ironclaw_safety = { workspace = true }

# 其他依赖保持不变...
```

#### 2.3 更新 Admin Backend 的 Cargo.toml

```toml
[dependencies]
# 使用工作空间依赖
ironclaw = { workspace = true }
ironclaw_auth = { workspace = true }

# 其他依赖保持不变...
```

### 阶段 3: 移除核心修改（1小时）

#### 3.1 移除测试文件

我们在 `src/safety/` 下添加的测试文件应该移动到合适的位置：

```bash
# 检查是否有我们添加的测试文件
ls -la src/safety/dlp_*_tests.rs 2>/dev/null

# 如果存在，移动到 desktop-client 或删除
# （因为 src/ 目录现在是 IronClaw submodule 的一部分）
```

#### 3.2 确认没有修改 IronClaw 核心

```bash
# 检查 ironclaw submodule 的状态
cd ironclaw
git status
git diff

# 如果有修改，重置到干净状态
git reset --hard HEAD
cd ..
```

### 阶段 4: 验证和测试（1小时）

#### 4.1 编译验证

```bash
# 检查工作空间配置
cargo check --workspace

# 编译所有组件
cargo build --workspace
```

#### 4.2 运行测试

```bash
# 运行所有测试
cargo test --workspace

# 分别测试各组件
cargo test -p ironclaw_auth
cargo test -p ironclaw_safety
cargo test -p desktop-client
cargo test -p admin-backend
```

### 阶段 5: 文档更新（30分钟）

#### 5.1 更新 README.md

```markdown
# IronClaw Suite

企业级 AI Agent 平台套件，基于 IronClaw 构建。

## 架构

本项目使用 Git Submodule 集成 IronClaw 核心：

- **ironclaw/**: IronClaw 核心（Git Submodule）
- **desktop-client/**: 桌面客户端应用
- **admin-backend/**: 管理后台应用
- **crates/**: 共享 Crate 模块

## 快速开始

### 克隆仓库

```bash
# 克隆仓库（包含 submodule）
git clone --recursive https://github.com/your-org/ironclaw-suite.git
cd ironclaw-suite

# 如果已经克隆，初始化 submodule
git submodule update --init --recursive
```

### 编译

```bash
cargo build --workspace
```

### 运行

```bash
# 启动 IronClaw 核心
cd ironclaw
cargo run -- run

# 启动 Admin Backend
cd admin-backend
cargo run

# 启动 Desktop Client
cd desktop-client
cargo tauri dev
```

## 更新 IronClaw

```bash
# 更新到最新版本
cd ironclaw
git pull origin main
cd ..
git add ironclaw
git commit -m "chore: update IronClaw submodule"
```

## License

MIT OR Apache-2.0
```

#### 5.2 创建架构文档

创建 `docs/ARCHITECTURE.md` 说明新的架构设计

### 阶段 6: Git 提交（15分钟）

```bash
# 添加所有更改
git add .

# 提交
git commit -m "refactor: 重构为 IronClaw Suite 架构

- 使用 IronClaw 作为 Git Submodule
- 更新工作空间配置
- 移除对 IronClaw 核心的修改
- 更新文档
"

# 推送
git push origin main
```

## 关键注意事项

### 1. Submodule 管理

**更新 IronClaw**:
```bash
cd ironclaw
git fetch
git checkout v0.19.0  # 或其他版本
cd ..
git add ironclaw
git commit -m "chore: update IronClaw to v0.19.0"
```

**团队协作**:
```bash
# 其他开发者克隆时
git clone --recursive <repo-url>

# 或者
git clone <repo-url>
git submodule update --init --recursive
```

### 2. 依赖管理

**不要修改 ironclaw/ 目录下的任何文件**
- 所有修改应该在 desktop-client/ 或 admin-backend/ 中
- 通过 Cargo.toml 的路径依赖使用 IronClaw

**如果需要扩展功能**:
- 在 crates/ 下创建新的共享 Crate
- 在应用层实现扩展逻辑
- 不要修改 IronClaw 核心

### 3. 版本控制

**推荐的 IronClaw 版本策略**:
- 开发环境：使用 `staging` 分支
- 生产环境：使用稳定的 tag（如 `v0.18.0`）

```bash
# 开发环境
cd ironclaw && git checkout staging && cd ..

# 生产环境
cd ironclaw && git checkout v0.18.0 && cd ..
```

## 时间估算

| 阶段 | 预计时间 | 说明 |
|------|----------|------|
| 1. 清理和准备 | 30分钟 | 检查和备份 |
| 2. 更新工作空间配置 | 1小时 | 修改 Cargo.toml |
| 3. 移除核心修改 | 1小时 | 清理不必要的文件 |
| 4. 验证和测试 | 1小时 | 编译和测试 |
| 5. 文档更新 | 30分钟 | 更新文档 |
| 6. Git 提交 | 15分钟 | 提交和推送 |
| **总计** | **4小时15分钟** | |

## 优势

相比原方案B，修订方案的优势：

1. ✅ **更简单**: 不需要创建新仓库和迁移代码
2. ✅ **更快速**: 总时间从 13.5 小时减少到 4.25 小时
3. ✅ **更安全**: 保留现有的 Git 历史
4. ✅ **已就绪**: IronClaw submodule 已经配置好
5. ✅ **易维护**: 标准的 Git Submodule 工作流

## 下一步

1. 执行阶段 1-6
2. 验证所有功能正常
3. 更新团队文档
4. 通知团队成员新的工作流程

## 参考文档

- [IronClaw 核心修改记录](IRONCLAW_CORE_MODIFICATIONS.md)
- [原方案B](ARCHITECTURE_REFACTOR_PLAN_B.md)
- [AGENTS.md](AGENTS.md) - 开发规则
