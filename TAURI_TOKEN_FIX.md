# Tauri 客户端 Token 认证修复

## 问题描述

Tauri 桌面客户端一直报 401 Unauthorized 错误，原因是：
- 后端每次启动都会生成新的认证令牌
- Tauri 客户端使用硬编码的令牌
- 令牌不匹配导致认证失败

## 解决方案

### 1. 修改 API 客户端读取环境变量

**文件**: `desktop-client/src/api_client.rs`

```rust
pub fn new(base_url: String) -> Self {
    // 优先从环境变量读取 token，如果没有则使用默认值
    let auth_token = std::env::var("GATEWAY_AUTH_TOKEN")
        .unwrap_or_else(|_| "59c7c863fa5bd3eeffc94533cd70a3393251c3ada49a226146a5a61ba62d6743".to_string());
    
    Self {
        base_url,
        client: reqwest::Client::new(),
        auth_token,
    }
}
```

### 2. 更新启动脚本自动提取令牌

**文件**: `scripts/start-all.sh`

添加了以下步骤：
1. 从后端日志中提取令牌
2. 导出到环境变量 `GATEWAY_AUTH_TOKEN`
3. 启动 Tauri 客户端（自动继承环境变量）

```bash
# 提取令牌
GATEWAY_URL=$(grep "gateway.*http" /tmp/backend.log | tail -1 | sed 's/.*http/http/')
export GATEWAY_AUTH_TOKEN=$(echo "$GATEWAY_URL" | sed 's/.*token=//')

# 启动 Tauri（会继承环境变量）
cd desktop-client
cargo tauri dev
```

### 3. 创建独立的 Tauri 启动脚本

**文件**: `scripts/test-tauri-token.sh`

用于单独启动 Tauri 客户端：
```bash
bash scripts/test-tauri-token.sh
```

## 使用方法

### 方法 1：一键启动所有服务（推荐）

```bash
export LLM_API_KEY="sk-d162b1775e514757b083a2dfe729d6e6"
bash scripts/start-all.sh
```

这会启动：
- 后端服务（端口 3000）
- 前端开发服务器（端口 5173）
- Tauri 桌面客户端（自动使用正确的令牌）

### 方法 2：单独启动 Tauri

如果后端和前端已经在运行：

```bash
bash scripts/test-tauri-token.sh
```

### 方法 3：手动设置令牌

```bash
# 1. 从后端日志获取 token
grep "token=" /tmp/backend.log | tail -1

# 2. 设置环境变量
export GATEWAY_AUTH_TOKEN="your-token-here"

# 3. 启动 Tauri
cd desktop-client
cargo tauri dev
```

## 技术细节

### Token 流程

```
后端启动
  ↓
生成新的认证令牌
  ↓
写入日志 /tmp/backend.log
  ↓
启动脚本提取令牌
  ↓
导出到环境变量 GATEWAY_AUTH_TOKEN
  ↓
Tauri 客户端读取环境变量
  ↓
使用正确的令牌进行 API 调用
```

### 环境变量优先级

Tauri 客户端的 token 获取优先级：
1. 环境变量 `GATEWAY_AUTH_TOKEN`（最高优先级）
2. 硬编码的默认值（fallback）

### 浏览器端 vs Tauri 端

| 特性 | 浏览器端 | Tauri 端 |
|------|---------|---------|
| Token 来源 | URL 参数 / localStorage | 环境变量 |
| 管理方式 | TokenManager 类 | 环境变量 |
| 自动保存 | ✅ localStorage | ❌ 不需要 |
| 更新方式 | URL 参数自动更新 | 重启时自动读取 |

## 验证

### 1. 检查环境变量

```bash
echo $GATEWAY_AUTH_TOKEN
```

### 2. 检查后端日志

```bash
grep "token=" /tmp/backend.log | tail -1
```

### 3. 测试 API 调用

在 Tauri 客户端中：
- 打开聊天标签页
- 应该能看到对话列表
- 不应该有 401 错误

## 相关文件

- `desktop-client/src/api_client.rs` - API 客户端（读取环境变量）
- `scripts/start-all.sh` - 一键启动脚本
- `scripts/test-tauri-token.sh` - Tauri 独立启动脚本
- `QUICK_START_GUIDE.md` - 快速启动指南
- `TAURI_TOKEN_ISSUE.md` - 问题分析文档

## 注意事项

1. **必须先编译**: 修改 `api_client.rs` 后需要重新编译
   ```bash
   cd desktop-client
   cargo build
   ```

2. **环境变量作用域**: 环境变量只在当前 shell 会话中有效

3. **后端重启**: 每次后端重启都会生成新令牌，需要重启 Tauri 客户端

4. **默认值**: 如果环境变量未设置，会使用硬编码的默认值（可能过期）

## 未来改进

1. **自动刷新**: Tauri 客户端检测到 401 错误时自动重新获取令牌
2. **配置文件**: 将令牌保存到配置文件中
3. **令牌过期检测**: 主动检测令牌是否过期
4. **统一管理**: 浏览器端和 Tauri 端使用相同的 token 管理机制
