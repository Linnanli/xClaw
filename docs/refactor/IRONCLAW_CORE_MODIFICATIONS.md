# IronClaw 核心代码修改记录

## 文档目的

本文档记录了我们（nally.lin）对 IronClaw 核心代码的所有修改，用于：
1. 架构重构时的参考
2. 功能迁移的指导
3. 版本管理的依据

## 修改概览

**作者**: nally.lin  
**修改时间**: 2024年至今  
**修改范围**: 主项目 `src/` 目录下的核心文件  
**修改性质**: 主要是功能扩展，少量 bug 修复

## 详细修改清单

### 1. 认证模块（已提取为共享 Crate）

**状态**: ✅ 已解决 - 已提取为 `crates/ironclaw_auth/`

#### 修改的文件
- `src/auth/jwt.rs` - JWT 认证逻辑
- `src/auth/mod.rs` - 认证模块入口
- `src/auth/password.rs` - 密码处理

#### 修改内容
- 实现了 JWT token 生成和验证
- 实现了 Argon2 密码哈希
- 添加了认证错误处理

#### 迁移策略
- ✅ 已完成：提取为 `crates/ironclaw_auth/` 共享 Crate
- ✅ Desktop Client 和 Admin Backend 都使用该共享 Crate
- ✅ 核心代码中的认证逻辑已被移除

#### 相关提交
- `1c5129c` - feat: 实现管理后台核心基础设施和共享认证模块
- `d386927` - refactor: 创建 ironclaw_auth 共享 crate 并迁移 admin-backend

---

### 2. 加密和安全模块

**状态**: ⚠️ 需要处理 - 扩展了加密能力

#### 修改的文件
- `src/secrets/crypto_provider.rs` - 加密提供者抽象（新增 212 行）
- `src/secrets/crypto_config.rs` - 加密配置（新增 136 行）
- `src/secrets/default_crypto.rs` - 默认加密实现（新增 351 行）
- `src/secrets/gm_crypto.rs` - 国密加密实现（新增 339 行）
- `src/secrets/mod.rs` - 安全模块入口（修改 13 行）

#### 修改内容
1. **加密提供者抽象**
   - 创建了 `CryptoProvider` trait
   - 支持多种加密算法的抽象接口
   - 提供了算法切换机制

2. **国密支持**
   - 实现了 SM2/SM3/SM4 国密算法
   - 符合中国密码标准
   - 提供了与默认加密的兼容层

3. **配置管理**
   - 支持通过配置选择加密算法
   - 提供了加密算法的运行时切换

#### 修改原因
- 满足中国市场的合规要求
- 提供更灵活的加密选择
- 支持企业级加密需求

#### 迁移策略（方案B）
**选项 1: 在新仓库中重新实现**
- 在 `crates/ironclaw_crypto/` 创建独立的加密扩展 Crate
- 不修改 IronClaw 核心代码
- Desktop Client 和 Admin Backend 通过依赖该 Crate 使用国密

**选项 2: 通过插件机制实现**
- 将国密支持作为可选插件
- 运行时动态加载
- 完全解耦核心代码

**推荐**: 选项 1 - 创建独立的加密扩展 Crate

#### 相关提交
- `5b0a7bd` - feat: 添加加密提供者抽象和属性测试支持

#### 相关测试
- `src/secrets/crypto_algorithm_consistency_tests.rs`
- `src/secrets/crypto_provider_tests.rs`
- `src/secrets/crypto_roundtrip_tests.rs`

---

### 3. 启动和引导模块

**状态**: ⚠️ 需要评估 - 小幅修改

#### 修改的文件
- `src/bootstrap.rs` - 启动逻辑（修改 4 行）

#### 修改内容
- 调整了启动流程
- 可能与加密初始化相关

#### 迁移策略（方案B）
- 检查修改的必要性
- 如果是加密相关，可以在新架构中通过配置实现
- 如果是 bug 修复，可以提交 PR 到上游

#### 相关提交
- `5b0a7bd` - feat: 添加加密提供者抽象和属性测试支持

---

### 4. Web 服务器模块

**状态**: ⚠️ 需要评估 - 小幅修改

#### 修改的文件
- `src/channels/web/server.rs` - Web 服务器（修改 9 行）
- `src/channels/webhook_server.rs` - Webhook 服务器（修改 9 行）

#### 修改内容
- 可能与加密或认证集成相关
- 小幅调整服务器配置

#### 迁移策略（方案B）
- 检查修改的具体内容
- 评估是否可以通过配置或中间件实现
- 如果是 bug 修复，可以提交 PR 到上游

#### 相关提交
- `5b0a7bd` - feat: 添加加密提供者抽象和属性测试支持

---

### 5. 文档提取模块

**状态**: ✅ 可以重新实现 - 功能改进

#### 修改的文件
- `src/document_extraction/extractors.rs` - PDF 提取器

#### 修改内容
- 为 PDF 提取添加了容错回退机制
- 改进了错误处理

#### 迁移策略（方案B）
- 这是功能改进，不是核心依赖
- 可以在新架构中重新实现
- 或者提交 PR 到上游

#### 相关提交
- `666e3e1` - feat: 为PDF提取添加容错回退机制

---

### 6. 配对和存储模块

**状态**: ⚠️ 需要评估 - 小幅修改

#### 修改的文件
- `src/cli/pairing.rs` - 配对命令（修改 2 行）
- `src/pairing/store.rs` - 配对存储（修改 7 行）

#### 修改内容
- 修复了文件锁定竞争条件
- 改进了配对存储逻辑

#### 迁移策略（方案B）
- 这是 bug 修复
- 应该提交 PR 到上游
- 在新架构中使用上游修复后的版本

#### 相关提交
- `c10d6ce` - fix(pairing): 修复文件锁定竞争条件和改进测试

---

### 7. 工具模块

**状态**: ⚠️ 需要评估 - 小幅修改

#### 修改的文件
- `src/tools/builtin/http.rs` - HTTP 工具（修改 68 行）
- `src/tools/builtin/message.rs` - 消息工具（修改 15 行）

#### 修改内容
- 可能与加密或安全相关
- 工具功能的改进

#### 迁移策略（方案B）
- 检查修改的具体内容
- 评估是否可以通过扩展机制实现
- 如果是通用改进，提交 PR 到上游

#### 相关提交
- `5b0a7bd` - feat: 添加加密提供者抽象和属性测试支持

---

### 8. 服务模块

**状态**: ⚠️ 需要评估 - 小幅修改

#### 修改的文件
- `src/service.rs` - 服务入口（修改 6 行）

#### 修改内容
- 可能与加密初始化相关
- 服务启动流程的调整

#### 迁移策略（方案B）
- 检查修改的具体内容
- 评估是否可以通过配置实现
- 如果是架构改进，提交 PR 到上游

#### 相关提交
- `5b0a7bd` - feat: 添加加密提供者抽象和属性测试支持

---

### 9. 库入口模块

**状态**: ✅ 可以重新实现 - 模块导出

#### 修改的文件
- `src/lib.rs` - 库入口

#### 修改内容
- 添加了新模块的导出
- 主要是 `pub mod` 声明

#### 迁移策略（方案B）
- 这些导出在新架构中不需要
- 使用纯净的 IronClaw 即可

#### 相关提交
- 多个提交涉及模块导出

---

## 测试文件修改

### DLP 测试文件（在主项目中）

**状态**: ⚠️ 需要迁移 - 测试文件

#### 修改的文件
- `src/safety/dlp_rule_matching_tests.rs` - DLP 规则匹配测试
- `src/safety/dlp_sanitization_property_tests.rs` - DLP 脱敏属性测试

#### 迁移策略（方案B）
- 这些测试文件应该移动到 `desktop-client/` 或 `crates/ironclaw_safety/`
- 不应该在主项目的 `src/` 目录中

---

## 工作空间配置修改

### Cargo.toml

**状态**: ✅ 需要恢复 - 配置文件

#### 修改内容
```toml
[workspace]
members = [".", "desktop-client", "admin-backend", "crates/ironclaw_safety", "crates/ironclaw_auth"]
```

#### 迁移策略（方案B）
- 恢复 IronClaw 原始的 `Cargo.toml`
- 在新仓库中创建新的工作空间配置

---

## 共享 Crate

### 我们创建的共享 Crate

**状态**: ✅ 需要迁移 - 独立模块

#### Crate 列表
1. `crates/ironclaw_auth/` - 认证模块
2. `crates/ironclaw_safety/` - 安全和 DLP 模块

#### 迁移策略（方案B）
- 将这些 Crate 移动到新仓库的 `crates/` 目录
- 保持独立性，不依赖 IronClaw 核心

---

## 方案B执行计划

### 阶段 1: 准备工作（1 小时）

1. **创建新仓库**
   ```bash
   mkdir ironclaw-suite
   cd ironclaw-suite
   git init
   ```

2. **添加 IronClaw 子模块**
   ```bash
   git submodule add https://github.com/nearai/ironclaw.git ironclaw
   ```

3. **创建基础结构**
   ```bash
   mkdir -p crates
   mkdir -p desktop-client
   mkdir -p admin-backend
   ```

### 阶段 2: 迁移共享 Crate（1 小时）

1. **迁移 ironclaw_auth**
   ```bash
   cp -r ../x-claw/crates/ironclaw_auth crates/
   ```

2. **迁移 ironclaw_safety**
   ```bash
   cp -r ../x-claw/crates/ironclaw_safety crates/
   ```

3. **创建加密扩展 Crate**
   ```bash
   # 将国密支持提取为独立 Crate
   mkdir -p crates/ironclaw_crypto
   # 实现加密扩展
   ```

### 阶段 3: 迁移应用层（2 小时）

1. **迁移 Desktop Client**
   ```bash
   cp -r ../x-claw/desktop-client .
   # 更新依赖路径
   ```

2. **迁移 Admin Backend**
   ```bash
   cp -r ../x-claw/admin-backend .
   # 更新依赖路径
   ```

3. **更新依赖配置**
   - 修改 `desktop-client/Cargo.toml`
   - 修改 `admin-backend/Cargo.toml`
   - 指向新的路径结构

### 阶段 4: 重新实现必要功能（2-3 小时）

1. **国密支持**
   - 在 `crates/ironclaw_crypto/` 中实现
   - 提供与 IronClaw 的集成接口

2. **PDF 提取改进**
   - 在 Desktop Client 或 Admin Backend 中实现
   - 或者提交 PR 到 IronClaw 上游

3. **其他小幅改进**
   - 评估每个修改的必要性
   - 通过配置或扩展机制实现

### 阶段 5: 测试和验证（1-2 小时）

1. **编译验证**
   ```bash
   cargo build --workspace
   ```

2. **运行测试**
   ```bash
   cargo test --workspace
   ```

3. **功能验证**
   - 测试 Desktop Client
   - 测试 Admin Backend
   - 测试 DLP 功能

### 阶段 6: 文档和清理（1 小时）

1. **更新文档**
   - README.md
   - 架构文档
   - 迁移指南

2. **清理旧代码**
   - 删除不需要的文件
   - 整理目录结构

---

## 总结

### 核心修改统计

| 类别 | 文件数 | 新增行数 | 修改行数 | 状态 |
|------|--------|----------|----------|------|
| 认证模块 | 3 | ~500 | - | ✅ 已提取 |
| 加密模块 | 5 | ~1200 | ~20 | ⚠️ 需要重新实现 |
| 启动模块 | 1 | - | 4 | ⚠️ 需要评估 |
| Web 服务器 | 2 | - | 18 | ⚠️ 需要评估 |
| 文档提取 | 1 | ~50 | - | ✅ 可以重新实现 |
| 配对存储 | 2 | - | 9 | ⚠️ 需要评估 |
| 工具模块 | 2 | - | 83 | ⚠️ 需要评估 |
| 服务模块 | 1 | - | 6 | ⚠️ 需要评估 |
| 库入口 | 1 | - | ~10 | ✅ 可以重新实现 |

### 迁移成本估算

- **总时间**: 8-10 小时
- **风险等级**: 中等
- **技术难度**: 中等

### 关键决策点

1. **国密支持**: 是否需要？如何实现？
2. **小幅修改**: 哪些提交 PR 到上游？哪些在新架构中实现？
3. **测试覆盖**: 如何确保功能完整性？

### 下一步行动

1. ✅ 记录所有修改（本文档）
2. ⏳ 创建新仓库结构
3. ⏳ 迁移共享 Crate
4. ⏳ 迁移应用层
5. ⏳ 重新实现必要功能
6. ⏳ 测试和验证
7. ⏳ 文档和清理

---

## 附录

### 相关文档
- `AGENTS.md` - 开发规则和测试策略
- `DESKTOP_CLIENT_FEATURE_CHECKLIST.md` - Desktop Client 功能清单
- `DLP_*.md` - DLP 相关文档

### Git 提交记录
```bash
# 查看所有 nally.lin 的提交
git log --author="nally.lin" --oneline

# 查看特定文件的修改历史
git log --author="nally.lin" --follow -- src/secrets/crypto_provider.rs
```

### 联系方式
- 作者: nally.lin
- 日期: 2024年
- 项目: IronClaw Suite
