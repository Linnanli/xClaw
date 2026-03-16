# 脚本清理指南

## 脚本分析

### 启动和设置脚本

| 脚本 | 用途 | 状态 | 建议 |
|------|------|------|------|
| `scripts/start-all.sh` | 启动后端和前端 | ✅ 最新 | **保留** - 这是主要的启动脚本 |
| `scripts/start-backend-with-qwen.sh` | 仅启动后端 | ⚠️ 过时 | **删除** - 功能已被 `start-all.sh` 包含 |
| `scripts/setup-qwen-default.sh` | 设置 Qwen 为默认 | ⚠️ 过时 | **删除** - 功能已被 `start-all.sh` 包含 |
| `scripts/test-qwen-e2e.sh` | 运行 E2E 测试 | ⚠️ 过时 | **删除** - 使用 `cargo test` 更好 |

### 其他脚本

| 脚本 | 用途 | 保留 |
|------|------|------|
| `scripts/build-all.sh` | 构建所有项目 | ✅ 保留 |
| `scripts/build-wasm-extensions.sh` | 构建 WASM 扩展 | ✅ 保留 |
| `scripts/check-boundaries.sh` | 检查边界 | ✅ 保留 |
| `scripts/check-version-bumps.sh` | 检查版本 | ✅ 保留 |
| `scripts/ci/*` | CI 脚本 | ✅ 保留 |
| `scripts/code-quality-gate.sh` | 代码质量检查 | ✅ 保留 |
| `scripts/commit-msg-regression.sh` | 提交消息检查 | ✅ 保留 |
| `scripts/coverage.sh` | 覆盖率检查 | ✅ 保留 |
| `scripts/dev-setup.sh` | 开发环境设置 | ✅ 保留 |
| `scripts/pre-commit-safety.sh` | 提交前安全检查 | ✅ 保留 |
| `scripts/run-e2e-tests.sh` | 运行 E2E 测试 | ✅ 保留 |
| `scripts/scaffold-tdd-module.sh` | TDD 模块脚手架 | ✅ 保留 |
| `scripts/setup-rust-core.sh` | 设置 Rust 核心 | ✅ 保留 |
| `scripts/test-ci-artifact-naming.sh` | CI 工件命名测试 | ✅ 保留 |

---

## 删除的脚本

### 应删除的脚本 (3 个)

```bash
rm -f \
  scripts/start-backend-with-qwen.sh \
  scripts/setup-qwen-default.sh \
  scripts/test-qwen-e2e.sh
```

### 删除原因

1. **`start-backend-with-qwen.sh`**
   - 功能已被 `start-all.sh` 完全包含
   - `start-all.sh` 更完整，包括前端启动和错误处理
   - 保留会造成混淆

2. **`setup-qwen-default.sh`**
   - 功能已被 `start-all.sh` 包含
   - 环境变量设置已在 `start-all.sh` 中完成
   - 不需要单独的设置脚本

3. **`test-qwen-e2e.sh`**
   - 功能可以用 `cargo test` 直接完成
   - 不需要额外的包装脚本
   - 使用 `cargo test --test qwen_e2e_test` 更直接

---

## 保留的脚本

### 主要启动脚本
- ✅ `scripts/start-all.sh` - 启动后端和前端的主脚本

### 构建脚本
- ✅ `scripts/build-all.sh` - 构建所有项目
- ✅ `scripts/build-wasm-extensions.sh` - 构建 WASM 扩展

### 检查脚本
- ✅ `scripts/check-boundaries.sh` - 检查边界
- ✅ `scripts/check-version-bumps.sh` - 检查版本
- ✅ `scripts/code-quality-gate.sh` - 代码质量检查
- ✅ `scripts/commit-msg-regression.sh` - 提交消息检查
- ✅ `scripts/coverage.sh` - 覆盖率检查
- ✅ `scripts/pre-commit-safety.sh` - 提交前安全检查

### 设置脚本
- ✅ `scripts/dev-setup.sh` - 开发环境设置
- ✅ `scripts/setup-rust-core.sh` - 设置 Rust 核心

### 测试脚本
- ✅ `scripts/run-e2e-tests.sh` - 运行 E2E 测试
- ✅ `scripts/scaffold-tdd-module.sh` - TDD 模块脚手架
- ✅ `scripts/test-ci-artifact-naming.sh` - CI 工件命名测试

### CI 脚本
- ✅ `scripts/ci/delta_lint.sh` - Delta lint
- ✅ `scripts/ci/quality_gate.sh` - 质量门禁
- ✅ `scripts/ci/quality_gate_strict.sh` - 严格质量门禁

---

## 清理命令

```bash
# 删除过时的脚本
rm -f \
  scripts/start-backend-with-qwen.sh \
  scripts/setup-qwen-default.sh \
  scripts/test-qwen-e2e.sh

# 验证删除
ls -la scripts/*.sh | wc -l
```

---

## 清理后的脚本列表

清理后应该有 17 个脚本：

```
scripts/build-all.sh
scripts/build-wasm-extensions.sh
scripts/check-boundaries.sh
scripts/check-version-bumps.sh
scripts/ci/delta_lint.sh
scripts/ci/quality_gate.sh
scripts/ci/quality_gate_strict.sh
scripts/code-quality-gate.sh
scripts/commit-msg-regression.sh
scripts/coverage.sh
scripts/dev-setup.sh
scripts/pre-commit-safety.sh
scripts/run-e2e-tests.sh
scripts/scaffold-tdd-module.sh
scripts/setup-rust-core.sh
scripts/start-all.sh
scripts/test-ci-artifact-naming.sh
```

---

## 使用指南

### 启动开发环境

```bash
export LLM_API_KEY="sk-d162b1775e514757b083a2dfe729d6e6"
bash scripts/start-all.sh
```

### 运行 E2E 测试

```bash
cd desktop-client
cargo test --test qwen_e2e_test diagnose_qwen_setup -- --nocapture --ignored
```

### 构建项目

```bash
bash scripts/build-all.sh
```

### 代码质量检查

```bash
bash scripts/code-quality-gate.sh
```

