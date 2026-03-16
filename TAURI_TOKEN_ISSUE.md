# Tauri 令牌问题

## 问题

Tauri 应用使用硬编码的认证令牌，每次后端重启时令牌都会改变，导致 Tauri 应用无法连接。

## 根本原因

### 代码位置

`desktop-client/src/api_client.rs`:

```rust
impl ApiClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::new(),
            auth_token: "硬编码的令牌".to_string(), // ❌ 问题在这里
        }
    }
}
```

### 问题

1. 令牌是硬编码的
2. 每次后端重启都会生成新令牌
3. Tauri 应用需要重新编译才能使用新令牌

---

## 临时解决方案

### 步骤 1: 获取新令牌

```bash
grep "gateway" /tmp/backend.log | tail -1
```

输出示例：
```
gateway   http://127.0.0.1:3000/?token=59c7c863fa5bd3eeffc94533cd70a3393251c3ada49a226146a5a61ba62d6743
```

### 步骤 2: 更新代码

编辑 `desktop-client/src/api_client.rs`：

```rust
impl ApiClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::new(),
            auth_token: "59c7c863fa5bd3eeffc94533cd70a3393251c3ada49a226146a5a61ba62d6743".to_string(),
        }
    }
}
```

### 步骤 3: 重新编译

```bash
cd desktop-client
cargo tauri dev
```

---

## 长期解决方案

### 方案 1: 从环境变量读取令牌

修改 `desktop-client/src/api_client.rs`：

```rust
impl ApiClient {
    pub fn new(base_url: String) -> Self {
        let auth_token = std::env::var("GATEWAY_AUTH_TOKEN")
            .unwrap_or_else(|_| "default_token".to_string());
        
        Self {
            base_url,
            client: reqwest::Client::new(),
            auth_token,
        }
    }
}
```

然后启动时设置环境变量：

```bash
export GATEWAY_AUTH_TOKEN="59c7c863fa5bd3eeffc94533cd70a3393251c3ada49a226146a5a61ba62d6743"
cd desktop-client
cargo tauri dev
```

### 方案 2: 从配置文件读取令牌

修改 `desktop-client/src/api_client.rs`：

```rust
impl ApiClient {
    pub fn new(base_url: String) -> Self {
        let auth_token = Self::load_token_from_config()
            .unwrap_or_else(|_| "default_token".to_string());
        
        Self {
            base_url,
            client: reqwest::Client::new(),
            auth_token,
        }
    }
    
    fn load_token_from_config() -> Result<String> {
        let config_path = dirs::config_dir()
            .ok_or_else(|| Error::msg("Config dir not found"))?
            .join("ironclaw")
            .join("token.txt");
        
        std::fs::read_to_string(config_path)
            .map(|s| s.trim().to_string())
            .map_err(|e| Error::msg(format!("Failed to read token: {}", e)))
    }
}
```

然后创建配置文件：

```bash
mkdir -p ~/.config/ironclaw
echo "59c7c863fa5bd3eeffc94533cd70a3393251c3ada49a226146a5a61ba62d6743" > ~/.config/ironclaw/token.txt
```

### 方案 3: 添加 Tauri 命令更新令牌

添加一个 Tauri 命令来动态更新令牌：

```rust
#[tauri::command]
pub async fn update_auth_token(
    token: String,
    state: tauri::State<'_, CommandState>,
) -> Result<()> {
    // 更新 API 客户端的令牌
    // 需要修改 ApiClient 支持动态更新
    Ok(())
}
```

### 方案 4: 从后端自动获取令牌

在 Tauri 应用启动时自动从后端获取令牌：

```rust
impl ApiClient {
    pub async fn new_with_auto_token(base_url: String) -> Result<Self> {
        // 1. 尝试从本地存储读取令牌
        let token = Self::load_token_from_storage()
            .or_else(|_| {
                // 2. 如果没有，从后端获取
                Self::fetch_token_from_backend(&base_url)
            })?;
        
        Ok(Self {
            base_url,
            client: reqwest::Client::new(),
            auth_token: token,
        })
    }
}
```

---

## 推荐方案

### 短期（立即可用）

使用**方案 1: 从环境变量读取令牌**

优点：
- ✅ 简单易实现
- ✅ 不需要重新编译
- ✅ 可以在启动脚本中设置

缺点：
- ❌ 需要手动设置环境变量

### 长期（最佳实践）

使用**方案 4: 从后端自动获取令牌**

优点：
- ✅ 完全自动化
- ✅ 无需手动配置
- ✅ 令牌自动同步

缺点：
- ❌ 需要更多开发工作
- ❌ 需要后端支持

---

## 实现步骤（方案 1）

### 1. 修改 ApiClient

编辑 `desktop-client/src/api_client.rs`：

```rust
impl ApiClient {
    pub fn new(base_url: String) -> Self {
        let auth_token = std::env::var("GATEWAY_AUTH_TOKEN")
            .unwrap_or_else(|_| {
                eprintln!("⚠️  GATEWAY_AUTH_TOKEN not set, using default");
                "default_token".to_string()
            });
        
        println!("🔑 Using auth token: {}...", &auth_token[..8]);
        
        Self {
            base_url,
            client: reqwest::Client::new(),
            auth_token,
        }
    }
}
```

### 2. 更新启动脚本

编辑 `scripts/start-all.sh`，在启动 Tauri 前设置环境变量：

```bash
# 提取令牌
TOKEN=$(grep "gateway.*http" /tmp/backend.log | tail -1 | sed 's/.*token=//' | sed 's/$//')

# 设置环境变量
export GATEWAY_AUTH_TOKEN="$TOKEN"

# 启动 Tauri
cargo tauri dev &
```

### 3. 测试

```bash
export LLM_API_KEY="sk-..."
bash scripts/start-all.sh
```

---

## 相关文件

- `desktop-client/src/api_client.rs` - API 客户端
- `desktop-client/src/commands.rs` - Tauri 命令
- `scripts/start-all.sh` - 启动脚本
- `TOKEN_UPDATE_INSTRUCTIONS.md` - 令牌更新说明

---

## 总结

| 方案 | 难度 | 效果 | 推荐 |
|------|------|------|------|
| 硬编码 | 简单 | 差 | ❌ |
| 环境变量 | 简单 | 好 | ✅ 短期 |
| 配置文件 | 中等 | 好 | ✅ 中期 |
| Tauri 命令 | 中等 | 很好 | ✅ 中期 |
| 自动获取 | 复杂 | 最好 | ✅ 长期 |

