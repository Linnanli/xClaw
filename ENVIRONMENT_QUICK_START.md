# 环境一致性快速开始指南

## 问题回顾

当前项目的环境不一致问题涉及多个层面：

```
浏览器端 ✅ 已解决
    ↓
Tauri 应用 ⚠️ 高优先级 → 已实现解决方案
    ↓
后端服务 ⚠️ 高优先级 → 已实现解决方案
    ↓
网络配置 ⚠️ 中优先级 → 已实现解决方案
    ↓
基础设施 ⚠️ 低优先级 → 已实现解决方案
```

## 已实现的解决方案

### 1. 认证令牌管理 ✅

**问题**：每次启动生成新令牌，前端无法连接

**解决方案**：
```rust
use ironclaw_desktop::AuthTokenManager;

let manager = AuthTokenManager::new();
let token = manager.load_or_generate()?;  // 自动持久化
```

**文件**：`desktop-client/src/auth_token_manager.rs`

### 2. 数据库配置 ✅

**问题**：仅支持 SQLite，生产环境需要 PostgreSQL

**解决方案**：
```rust
use ironclaw_desktop::DatabaseBackend;

// 自动从环境变量加载
let db = DatabaseBackend::from_env()?;

// 支持 SQLite 和 PostgreSQL
// DATABASE_URL=sqlite:///path/to/db.db
// DATABASE_URL=postgresql://user:pass@host/db
```

**文件**：`desktop-client/src/database_config.rs`

### 3. 应用配置 ✅

**问题**：配置值硬编码，不同环境配置不同

**解决方案**：
```rust
use ironclaw_desktop::AppConfig;

// 从 TOML 文件或环境变量加载
let config = AppConfig::load_or_default();
config.validate()?;
```

**文件**：`desktop-client/src/config_manager.rs`

### 4. 网络配置 ✅

**问题**：超时和重试设置不根据环境调整

**解决方案**：
```rust
use ironclaw_desktop::NetworkConfig;

// 自动选择环境特定配置
let config = NetworkConfig::from_env();

// 或使用预定义配置
let dev = NetworkConfig::development();
let prod = NetworkConfig::production();
```

**文件**：`desktop-client/src/network_config.rs`

### 5. 跨平台支持 ✅

**问题**：文件路径硬编码，仅支持 macOS

**解决方案**：
```rust
use ironclaw_desktop::platform_utils;

// 自动处理 macOS/Windows/Linux 路径差异
let db_path = platform_utils::get_database_path();
let config_path = platform_utils::get_config_file_path();
```

**文件**：`desktop-client/src/platform_utils.rs`

## 立即可采取的行动

### 第一步：复制环境变量模板

```bash
cp desktop-client/.env.example desktop-client/.env.development
cp desktop-client/.env.example desktop-client/.env.production
```

### 第二步：在 Tauri 应用中初始化

编辑 `desktop-client/src-tauri/src/main.rs`：

```rust
use ironclaw_desktop::{AuthTokenManager, AppConfig, EnvironmentChecker};

fn main() {
    // 检查环境一致性
    let mut checker = EnvironmentChecker::new();
    if !checker.run_all_checks() {
        eprintln!("Environment check failed!");
        checker.print_results();
        std::process::exit(1);
    }
    
    // 初始化认证令牌
    let token_manager = AuthTokenManager::new();
    let token = token_manager.load_or_generate()
        .expect("Failed to initialize auth token");
    
    // 加载应用配置
    let config = AppConfig::load_or_default();
    config.validate()
        .expect("Invalid configuration");
    
    println!("✓ Environment initialized successfully");
    println!("✓ Auth token: {}", &token[..8]);
    println!("✓ API URL: {}", config.api_base_url);
    
    // 继续启动应用
    tauri::Builder::default()
        // ...
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### 第三步：在 API 客户端中使用

编辑 `desktop-client/src/api_client.rs`：

```rust
use ironclaw_desktop::{AuthTokenManager, NetworkConfig};

pub struct ApiClient {
    base_url: String,
    token: String,
    network_config: NetworkConfig,
}

impl ApiClient {
    pub fn new(base_url: String) -> Result<Self, Box<dyn std::error::Error>> {
        let token_manager = AuthTokenManager::new();
        let token = token_manager.load_or_generate()?;
        let network_config = NetworkConfig::from_env();
        
        Ok(Self {
            base_url,
            token,
            network_config,
        })
    }
}
```

### 第四步：测试

```bash
# 开发环境
cd desktop-client
cargo build

# 测试认证令牌管理
cargo test auth_token_manager

# 测试数据库配置
cargo test database_config

# 测试应用配置
cargo test config_manager

# 测试网络配置
cargo test network_config
```

## 环境变量配置

### 开发环境 (.env.development)

```bash
ENVIRONMENT=development
API_BASE_URL=http://localhost:3000
API_TIMEOUT_SECS=30
DATABASE_TYPE=sqlite
LOG_LEVEL=debug
NETWORK_VERIFY_SSL=false
```

### 生产环境 (.env.production)

```bash
ENVIRONMENT=production
API_BASE_URL=https://api.example.com
API_TIMEOUT_SECS=10
DATABASE_TYPE=postgresql
DATABASE_URL=postgresql://user:pass@db.example.com/ironclaw
LOG_LEVEL=warn
NETWORK_VERIFY_SSL=true
```

## 验证环境一致性

运行环境检查工具：

```rust
use ironclaw_desktop::EnvironmentChecker;

let mut checker = EnvironmentChecker::new();
checker.run_all_checks();
checker.print_results();
```

输出示例：

```
=== Environment Consistency Check ===
Environment: Development
API URL: http://localhost:3000

✓ API Connectivity: API URL configured: http://localhost:3000
✓ CORS Configuration: CORS origins configured: ["http://localhost:5173", "http://127.0.0.1:5173"]
✓ Auth Method: Auth method: Both
✓ Environment Variables: All required environment variables are set
✓ Dependencies: All required dependencies are available

Passed: 5/5
```

## 常见问题

### Q: 如何在 Windows 上运行？

A: 路径会自动处理，无需修改代码：

```rust
// 在 Windows 上自动使用 C:\Users\<user>\AppData\Local\ironclaw
let path = platform_utils::get_database_path();
```

### Q: 如何切换到 PostgreSQL？

A: 设置环境变量：

```bash
export DATABASE_TYPE=postgresql
export DATABASE_URL=postgresql://user:pass@localhost:5432/ironclaw
```

或在 `.env` 文件中：

```
DATABASE_TYPE=postgresql
DATABASE_URL=postgresql://user:pass@localhost:5432/ironclaw
```

### Q: 如何禁用 SSL 验证（开发环境）？

A: 设置环境变量：

```bash
export NETWORK_VERIFY_SSL=false
```

### Q: 令牌存储在哪里？

A: 根据操作系统自动存储：

- macOS: `~/Library/Application Support/ironclaw/.auth_token`
- Windows: `C:\Users\<user>\AppData\Local\ironclaw\.auth_token`
- Linux: `~/.local/share/ironclaw/.auth_token`

## 下一步

1. **集成到 Tauri 应用** (今天)
   - 在 `src-tauri/src/main.rs` 中添加初始化代码
   - 测试在 macOS 上运行

2. **跨平台测试** (本周)
   - 在 Windows 上测试
   - 在 Linux 上测试

3. **生产部署** (下周)
   - 配置生产环境变量
   - 测试 PostgreSQL 连接
   - 部署到生产环境

## 相关文档

- `ENVIRONMENT_CONSISTENCY_IMPLEMENTATION.md` - 详细实现指南
- `ENVIRONMENT_ANALYSIS_SUMMARY.md` - 问题分析总结
- `desktop-client/.env.example` - 环境变量示例
- `desktop-client/src/auth_token_manager.rs` - 令牌管理器
- `desktop-client/src/database_config.rs` - 数据库配置
- `desktop-client/src/config_manager.rs` - 应用配置
- `desktop-client/src/network_config.rs` - 网络配置
- `desktop-client/src/platform_utils.rs` - 跨平台工具

## 总结

✅ **已实现的解决方案**：
- 认证令牌自动管理
- 多数据库支持
- 环境特定配置
- 网络配置优化
- 跨平台兼容性

🎯 **立即行动**：
1. 在 Tauri 应用中集成初始化代码
2. 在 API 客户端中使用配置管理器
3. 在 Windows 和 Linux 上测试

📈 **预期收益**：
- 支持 macOS、Windows、Linux
- 自动令牌管理
- 灵活的数据库配置
- 环境特定的网络优化
- 生产环境就绪
