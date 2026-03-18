# 架构重构执行计划（方案B）

## 目标

将 Desktop Client 和 Admin Backend 抽出为独立仓库，IronClaw 作为纯净的子模块，不修改其核心代码。

## 原则

1. **零核心修改**: 不修改 IronClaw 核心代码
2. **功能完整**: 保持所有现有功能
3. **清晰边界**: 明确的架构分层
4. **易于维护**: 便于跟踪上游更新

## 新仓库结构

```
ironclaw-suite/
├── ironclaw/                    # IronClaw 子模块（纯净）
│   ├── src/                     # 核心代码（不修改）
│   ├── Cargo.toml              # 原始配置
│   └── ...
├── desktop-client/             # 桌面客户端
│   ├── src/
│   ├── src-ui/
│   └── Cargo.toml
├── admin-backend/              # 管理后台
│   ├── src/
│   └── Cargo.toml
├── crates/                     # 共享 Crate
│   ├── ironclaw_auth/         # 认证模块
│   ├── ironclaw_safety/       # 安全和 DLP 模块
│   └── ironclaw_crypto/       # 加密扩展（新增）
├── Cargo.toml                  # 套件工作空间配置
├── README.md
└── docs/
    ├── ARCHITECTURE.md
    └── MIGRATION_GUIDE.md
```

## 执行步骤

详见后续章节...


## 阶段 1: 准备新仓库（预计 1 小时）

### 1.1 创建新仓库

```bash
# 创建新目录
mkdir ironclaw-suite
cd ironclaw-suite

# 初始化 Git
git init

# 创建 .gitignore
cat > .gitignore << 'EOF'
target/
Cargo.lock
.env
.DS_Store
node_modules/
dist/
EOF
```

### 1.2 添加 IronClaw 子模块

```bash
# 添加 IronClaw 作为子模块
git submodule add https://github.com/nearai/ironclaw.git ironclaw

# 或者使用特定版本
cd ironclaw
git checkout v0.18.0  # 使用我们基于的版本
cd ..
```

### 1.3 创建基础目录结构

```bash
mkdir -p crates/ironclaw_auth
mkdir -p crates/ironclaw_safety
mkdir -p crates/ironclaw_crypto
mkdir -p desktop-client
mkdir -p admin-backend
mkdir -p docs
mkdir -p scripts
```


## 阶段 2: 迁移共享 Crate（预计 2 小时）

### 2.1 迁移 ironclaw_auth

```bash
# 从旧仓库复制
cp -r ../x-claw/crates/ironclaw_auth crates/

# 验证 Cargo.toml
cat crates/ironclaw_auth/Cargo.toml
```

### 2.2 迁移 ironclaw_safety

```bash
# 从旧仓库复制
cp -r ../x-claw/crates/ironclaw_safety crates/

# 验证 Cargo.toml
cat crates/ironclaw_safety/Cargo.toml
```

### 2.3 创建 ironclaw_crypto（国密支持）

这是新创建的 Crate，用于替代对 IronClaw 核心的加密模块修改。

```bash
cd crates/ironclaw_crypto
cargo init --lib
```

创建 `crates/ironclaw_crypto/Cargo.toml`:
```toml
[package]
name = "ironclaw_crypto"
version = "0.1.0"
edition = "2024"
rust-version = "1.92"

[dependencies]
# 国密算法库
sm2 = "0.13"
sm3 = "0.4"
sm4 = "0.5"

# 标准加密库
aes-gcm = "0.10"
sha2 = "0.10"

# 错误处理
thiserror = "2"
anyhow = "1"

# 序列化
serde = { version = "1", features = ["derive"] }
```


## 阶段 3: 迁移应用层（预计 2 小时）

### 3.1 迁移 Desktop Client

```bash
# 复制整个 desktop-client 目录
cp -r ../x-claw/desktop-client .

# 更新 Cargo.toml 中的依赖路径
```

修改 `desktop-client/Cargo.toml`:
```toml
[dependencies]
# IronClaw 核心（作为库依赖）
ironclaw = { path = "../ironclaw" }

# 共享 Crate
ironclaw_auth = { path = "../crates/ironclaw_auth" }
ironclaw_safety = { path = "../crates/ironclaw_safety" }
ironclaw_crypto = { path = "../crates/ironclaw_crypto" }

# 其他依赖保持不变...
```

### 3.2 迁移 Admin Backend

```bash
# 复制整个 admin-backend 目录
cp -r ../x-claw/admin-backend .

# 更新 Cargo.toml 中的依赖路径
```

修改 `admin-backend/Cargo.toml`:
```toml
[dependencies]
# IronClaw 核心（作为库依赖）
ironclaw = { path = "../ironclaw" }

# 共享 Crate
ironclaw_auth = { path = "../crates/ironclaw_auth" }
ironclaw_safety = { path = "../crates/ironclaw_safety" }

# 其他依赖保持不变...
```

### 3.3 创建工作空间配置

创建根目录的 `Cargo.toml`:
```toml
[workspace]
members = [
    "ironclaw",
    "desktop-client",
    "admin-backend",
    "crates/ironclaw_auth",
    "crates/ironclaw_safety",
    "crates/ironclaw_crypto",
]

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
```


## 阶段 4: 实现加密扩展（预计 3 小时）

### 4.1 设计加密扩展接口

创建 `crates/ironclaw_crypto/src/lib.rs`:
```rust
//! 加密扩展模块
//! 
//! 提供国密算法支持，不修改 IronClaw 核心代码

pub mod provider;
pub mod gm;
pub mod error;

pub use provider::CryptoProvider;
pub use gm::GmCryptoProvider;
pub use error::CryptoError;

/// 加密算法类型
#[derive(Debug, Clone, Copy)]
pub enum CryptoAlgorithm {
    /// 默认加密（AES-GCM）
    Default,
    /// 国密算法（SM2/SM3/SM4）
    GuoMi,
}

/// 创建加密提供者
pub fn create_provider(algorithm: CryptoAlgorithm) -> Box<dyn CryptoProvider> {
    match algorithm {
        CryptoAlgorithm::Default => Box::new(DefaultCryptoProvider::new()),
        CryptoAlgorithm::GuoMi => Box::new(GmCryptoProvider::new()),
    }
}
```

### 4.2 实现国密提供者

创建 `crates/ironclaw_crypto/src/gm.rs`:
```rust
//! 国密加密提供者实现

use crate::provider::CryptoProvider;
use crate::error::CryptoError;

pub struct GmCryptoProvider {
    // 国密实现
}

impl GmCryptoProvider {
    pub fn new() -> Self {
        Self {}
    }
}

impl CryptoProvider for GmCryptoProvider {
    fn encrypt(&self, data: &[u8], key: &[u8]) -> Result<Vec<u8>, CryptoError> {
        // 使用 SM4 加密
        todo!("实现 SM4 加密")
    }
    
    fn decrypt(&self, data: &[u8], key: &[u8]) -> Result<Vec<u8>, CryptoError> {
        // 使用 SM4 解密
        todo!("实现 SM4 解密")
    }
}
```

### 4.3 集成到应用层

在 Desktop Client 和 Admin Backend 中使用:
```rust
use ironclaw_crypto::{create_provider, CryptoAlgorithm};

// 根据配置选择加密算法
let algorithm = if config.use_guomi {
    CryptoAlgorithm::GuoMi
} else {
    CryptoAlgorithm::Default
};

let crypto = create_provider(algorithm);
```


## 阶段 5: 处理其他核心修改（预计 2 小时）

### 5.1 PDF 提取改进

**选项 A**: 在应用层实现
```rust
// desktop-client/src/document_utils.rs
pub async fn extract_pdf_with_fallback(path: &str) -> Result<String> {
    // 实现容错回退机制
}
```

**选项 B**: 提交 PR 到 IronClaw 上游
- 创建 PR 描述
- 提交改进代码
- 等待合并后更新子模块

**推荐**: 选项 A（短期）+ 选项 B（长期）

### 5.2 配对存储修复

**处理方式**: 提交 PR 到上游
- 这是 bug 修复，应该贡献给社区
- 创建 issue 和 PR
- 在 PR 合并前，可以在应用层 workaround

### 5.3 其他小幅修改

**评估原则**:
1. 如果是 bug 修复 → 提交 PR 到上游
2. 如果是功能扩展 → 在应用层实现
3. 如果是配置调整 → 通过配置文件实现

**具体处理**:
- `src/bootstrap.rs` 修改 → 检查是否可以通过配置实现
- `src/channels/web/server.rs` 修改 → 检查是否可以通过中间件实现
- `src/tools/builtin/*.rs` 修改 → 检查是否可以通过扩展实现


## 阶段 6: 测试和验证（预计 2 小时）

### 6.1 编译验证

```bash
# 编译整个工作空间
cargo build --workspace

# 检查编译错误
cargo check --workspace

# 运行 clippy
cargo clippy --workspace
```

### 6.2 单元测试

```bash
# 运行所有单元测试
cargo test --workspace

# 运行特定模块的测试
cargo test -p ironclaw_auth
cargo test -p ironclaw_safety
cargo test -p ironclaw_crypto
cargo test -p desktop-client
cargo test -p admin-backend
```

### 6.3 集成测试

```bash
# Desktop Client 集成测试
cd desktop-client
cargo test --test '*_integration_tests'

# Admin Backend 集成测试
cd admin-backend
cargo test --test '*_integration_tests'
```

### 6.4 E2E 测试

```bash
# 启动后端
cd ironclaw
cargo run -- run --cli-only --no-onboard &

# 启动 Admin Backend
cd admin-backend
cargo run &

# 运行 Desktop Client E2E 测试
cd desktop-client
npm run test:e2e
```

### 6.5 功能验证清单

- [ ] Desktop Client 启动正常
- [ ] Admin Backend 启动正常
- [ ] 认证功能正常
- [ ] DLP 功能正常
- [ ] 记忆管理功能正常
- [ ] 聊天功能正常
- [ ] 任务管理功能正常
- [ ] 扩展管理功能正常
- [ ] 国密加密功能正常（如果启用）


## 阶段 7: 文档和清理（预计 1 小时）

### 7.1 创建 README.md

```markdown
# IronClaw Suite

企业级 AI Agent 平台套件，基于 IronClaw 构建。

## 架构

- **ironclaw/**: IronClaw 核心（子模块）
- **desktop-client/**: 桌面客户端应用
- **admin-backend/**: 管理后台应用
- **crates/**: 共享 Crate 模块

## 快速开始

### 前置要求
- Rust 1.92+
- Node.js 18+
- PostgreSQL 14+ 或 libSQL

### 安装

\`\`\`bash
# 克隆仓库（包含子模块）
git clone --recursive https://github.com/your-org/ironclaw-suite.git
cd ironclaw-suite

# 编译
cargo build --workspace
\`\`\`

### 运行

\`\`\`bash
# 启动 IronClaw 核心
cd ironclaw
cargo run -- run

# 启动 Admin Backend
cd admin-backend
cargo run

# 启动 Desktop Client
cd desktop-client
cargo tauri dev
\`\`\`

## 文档

- [架构文档](docs/ARCHITECTURE.md)
- [迁移指南](docs/MIGRATION_GUIDE.md)
- [开发指南](AGENTS.md)

## License

MIT OR Apache-2.0
```

### 7.2 创建架构文档

创建 `docs/ARCHITECTURE.md` - 详细的架构说明

### 7.3 创建迁移指南

创建 `docs/MIGRATION_GUIDE.md` - 从旧架构迁移的指南

### 7.4 更新 AGENTS.md

将 `AGENTS.md` 复制到新仓库，并更新相关路径

### 7.5 清理不需要的文件

```bash
# 删除旧的文档（如果不需要）
rm -f IRONCLAW_CORE_MODIFICATIONS.md  # 保留作为参考
rm -f ARCHITECTURE_REFACTOR_PLAN_B.md  # 保留作为参考

# 删除临时文件
find . -name "*.bak" -delete
find . -name ".DS_Store" -delete
```


## 阶段 8: Git 提交和发布（预计 30 分钟）

### 8.1 初始提交

```bash
# 添加所有文件
git add .

# 提交
git commit -m "feat: 初始化 IronClaw Suite 架构

- 添加 IronClaw 作为子模块
- 迁移 Desktop Client
- 迁移 Admin Backend
- 创建共享 Crate (ironclaw_auth, ironclaw_safety, ironclaw_crypto)
- 配置工作空间
"

# 添加远程仓库
git remote add origin https://github.com/your-org/ironclaw-suite.git

# 推送
git push -u origin main
```

### 8.2 创建标签

```bash
# 创建初始版本标签
git tag -a v0.1.0 -m "Initial release of IronClaw Suite"
git push origin v0.1.0
```

### 8.3 更新子模块

```bash
# 确保子模块正确初始化
git submodule update --init --recursive

# 推送子模块引用
git push --recurse-submodules=on-demand
```

## 总结

### 时间估算

| 阶段 | 预计时间 | 实际时间 |
|------|----------|----------|
| 1. 准备新仓库 | 1 小时 | |
| 2. 迁移共享 Crate | 2 小时 | |
| 3. 迁移应用层 | 2 小时 | |
| 4. 实现加密扩展 | 3 小时 | |
| 5. 处理其他修改 | 2 小时 | |
| 6. 测试和验证 | 2 小时 | |
| 7. 文档和清理 | 1 小时 | |
| 8. Git 提交和发布 | 0.5 小时 | |
| **总计** | **13.5 小时** | |

### 风险和缓解

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| 依赖路径错误 | 高 | 仔细检查所有 Cargo.toml |
| 功能缺失 | 高 | 完整的测试验证 |
| 性能下降 | 中 | 性能基准测试 |
| 文档不完整 | 中 | 详细的文档和示例 |

### 成功标准

- [ ] 所有代码编译通过
- [ ] 所有测试通过
- [ ] Desktop Client 功能完整
- [ ] Admin Backend 功能完整
- [ ] 文档完整清晰
- [ ] Git 历史清晰
- [ ] 子模块配置正确

### 后续工作

1. **持续集成**: 配置 CI/CD 流程
2. **性能优化**: 基准测试和优化
3. **文档完善**: 添加更多示例和教程
4. **社区贡献**: 将改进提交到 IronClaw 上游
5. **版本管理**: 建立清晰的版本发布流程

## 参考文档

- [IronClaw 核心修改记录](IRONCLAW_CORE_MODIFICATIONS.md)
- [AGENTS.md](AGENTS.md) - 开发规则
- [Desktop Client 功能清单](DESKTOP_CLIENT_FEATURE_CHECKLIST.md)
