# 环境不一致问题检查清单

## 已解决的问题 ✅

### 1. 浏览器端 SSE 连接
- [x] EventSource API 不支持自定义头
- [x] 实现 URL 参数认证
- [x] CORS 预检请求处理
- [x] 浏览器自动重连支持

**文件**：
- `desktop-client/src-ui/src/app/utils/sse.ts`
- `src/channels/web/server.rs`

---

## 需要解决的问题 ⚠️

### 2. Tauri 跨平台兼容性

#### 2.1 文件路径处理
- [ ] 统一文件路径管理
- [ ] 支持 macOS、Windows、Linux
- [ ] 处理路径分隔符差异
- [ ] 处理大小写敏感性差异

**优先级**：高
**文件**：`desktop-client/src/platform_utils.rs` (已创建)
**任务**：
```rust
// 使用 platform_utils 替换硬编码路径
// 旧代码：
let path = "/Users/nallylin/.ironclaw/ironclaw.db";

// 新代码：
let path = platform_utils::get_database_path();
```

#### 2.2 权限管理
- [ ] 文件系统访问权限
- [ ] 数据库访问权限
- [ ] 网络访问权限
- [ ] 系统资源访问权限

**优先级**：中
**任务**：
- 检查 Tauri 权限配置
- 实现权限检查机制
- 处理权限拒绝错误

#### 2.3 Tauri 命令和浏览器 API 混合
- [ ] 统一 API 调用方式
- [ ] 处理 CORS 限制
- [ ] 处理沙箱限制
- [ ] 错误处理统一

**优先级**：高
**任务**：
```typescript
// 当前混合使用：
// 1. 浏览器 API（受 CORS 限制）
const response = await fetch('http://localhost:3000/api/chat/threads');

// 2. Tauri 命令（直接调用）
const threads = await invoke('get_threads');

// 需要统一为一种方式
```

---

### 3. 后端认证令牌管理

#### 3.1 令牌生成和存储
- [ ] 令牌持久化存储
- [ ] 令牌过期管理
- [ ] 令牌轮换机制
- [ ] 令牌安全存储

**优先级**：高
**文件**：需要创建 `desktop-client/src/auth_token_manager.rs`
**任务**：
```rust
// 当前问题：每次启动生成新令牌
// 需要：从文件读取或生成一次后保存

pub struct AuthTokenManager {
    token_file: PathBuf,
}

impl AuthTokenManager {
    pub fn load_or_generate() -> Result<String> {
        let path = platform_utils::get_auth_token_path();
        
        if path.exists() {
            std::fs::read_to_string(&path)
        } else {
            let token = generate_random_token();
            std::fs::write(&path, &token)?;
            Ok(token)
        }
    }
}
```

#### 3.2 令牌同步
- [ ] 前端和后端令牌同步
- [ ] 多个客户端令牌管理
- [ ] 令牌刷新机制

**优先级**：中
**任务**：
- 实现令牌刷新 API
- 前端自动刷新令牌
- 处理令牌过期错误

---

### 4. 数据库配置

#### 4.1 多数据库支持
- [ ] SQLite（开发环境）
- [ ] PostgreSQL（生产环境）
- [ ] MySQL（可选）
- [ ] 数据库迁移

**优先级**：高
**任务**：
```rust
// 当前：只支持 SQLite
// 需要：支持多种数据库

pub enum DatabaseBackend {
    SQLite(String),
    PostgreSQL(String),
    MySQL(String),
}

impl DatabaseBackend {
    pub async fn connect(&self) -> Result<Arc<dyn Database>> {
        match self {
            DatabaseBackend::SQLite(path) => {
                // SQLite 连接
            }
            DatabaseBackend::PostgreSQL(url) => {
                // PostgreSQL 连接
            }
            DatabaseBackend::MySQL(url) => {
                // MySQL 连接
            }
        }
    }
}
```

#### 4.2 数据库配置管理
- [ ] 从环境变量读取
- [ ] 从配置文件读取
- [ ] 配置验证
- [ ] 连接池管理

**优先级**：高
**任务**：
- 创建 `database_config.rs`
- 实现配置加载和验证
- 实现连接池管理

---

### 5. 网络配置

#### 5.1 超时和重试
- [ ] 根据环境调整超时
- [ ] 根据环境调整重试次数
- [ ] 实现指数退避
- [ ] 处理超时错误

**优先级**：中
**任务**：
```rust
pub struct NetworkConfig {
    pub timeout: Duration,
    pub retry_count: u32,
    pub retry_delay: Duration,
}

impl NetworkConfig {
    pub fn from_environment() -> Self {
        match std::env::var("ENVIRONMENT").as_deref() {
            Ok("production") => Self::production(),
            Ok("testing") => Self::testing(),
            _ => Self::development(),
        }
    }
}
```

#### 5.2 连接管理
- [ ] 连接池配置
- [ ] 连接复用
- [ ] 连接超时
- [ ] 连接错误处理

**优先级**：中
**任务**：
- 配置 HTTP 客户端连接池
- 实现连接复用
- 处理连接错误

---

### 6. 依赖版本管理

#### 6.1 版本检查
- [ ] Rust 版本检查
- [ ] Node.js 版本检查
- [ ] 库版本检查
- [ ] 操作系统版本检查

**优先级**：低
**任务**：
```rust
pub fn check_dependencies() -> Result<()> {
    // 检查 Rust 版本
    let rustc_version = get_rustc_version()?;
    if rustc_version < Version::new(1, 70, 0) {
        return Err("Rust version too old".into());
    }
    
    // 检查其他依赖
    Ok(())
}
```

#### 6.2 版本锁定
- [ ] Cargo.lock 提交
- [ ] package-lock.json 提交
- [ ] 版本更新流程
- [ ] 安全补丁管理

**优先级**：低
**任务**：
- 确保 Cargo.lock 提交到 Git
- 确保 package-lock.json 提交到 Git
- 建立版本更新流程

---

### 7. 操作系统兼容性

#### 7.1 跨平台测试
- [ ] macOS 测试
- [ ] Windows 测试
- [ ] Linux 测试
- [ ] 文件系统测试

**优先级**：中
**任务**：
- 在三个平台上测试
- 记录平台特定问题
- 实现平台特定解决方案

#### 7.2 系统调用兼容性
- [ ] POSIX 调用
- [ ] Win32 调用
- [ ] 系统特定功能
- [ ] 权限管理

**优先级**：低
**任务**：
- 使用跨平台库（如 `std::fs`）
- 避免平台特定调用
- 使用条件编译处理差异

---

### 8. 配置管理

#### 8.1 配置文件
- [ ] 配置文件格式（TOML/YAML）
- [ ] 配置文件位置
- [ ] 配置文件验证
- [ ] 配置文件版本管理

**优先级**：高
**任务**：
```toml
# config/development.toml
[api]
base_url = "http://localhost:3000"
timeout = 30
retry = 3

[database]
backend = "libsql"
path = "~/.ironclaw/ironclaw.db"

[auth]
method = "both"
```

#### 8.2 环境变量
- [ ] 环境变量命名规范
- [ ] 环境变量文档
- [ ] 环境变量验证
- [ ] 默认值管理

**优先级**：高
**任务**：
- 创建 `.env.example` 文件
- 文档化所有环境变量
- 实现环境变量验证

---

### 9. 日志管理

#### 9.1 日志级别
- [ ] 开发环境：DEBUG
- [ ] 测试环境：INFO
- [ ] 生产环境：WARN
- [ ] 日志输出格式

**优先级**：低
**任务**：
```rust
pub fn init_logging() {
    let level = match std::env::var("ENVIRONMENT").as_deref() {
        Ok("production") => "warn",
        Ok("testing") => "info",
        _ => "debug",
    };
    
    tracing_subscriber::fmt()
        .with_max_level(level.parse().unwrap())
        .init();
}
```

#### 9.2 日志输出
- [ ] 控制台输出
- [ ] 文件输出
- [ ] 日志轮转
- [ ] 日志聚合

**优先级**：低
**任务**：
- 配置日志输出目标
- 实现日志轮转
- 集成日志聚合服务

---

### 10. 测试环境

#### 10.1 单元测试
- [ ] Mock 对象
- [ ] 隔离测试
- [ ] 快速执行
- [ ] 覆盖率检查

**优先级**：中
**任务**：
- 使用 `mockall` 创建 Mock 对象
- 实现单元测试
- 检查覆盖率

#### 10.2 集成测试
- [ ] 真实服务
- [ ] 集成测试
- [ ] 较慢执行
- [ ] 网络测试

**优先级**：中
**任务**：
- 使用 `testcontainers` 启动真实服务
- 实现集成测试
- 测试网络交互

#### 10.3 端到端测试
- [ ] 真实应用
- [ ] 完整流程
- [ ] UI 测试
- [ ] 用户场景

**优先级**：低
**任务**：
- 使用 Playwright 或 Cypress
- 实现端到端测试
- 测试用户场景

---

## 优先级排序

### 第一阶段（立即）
1. Tauri 文件路径处理 - `platform_utils.rs` ✅ 已创建
2. 后端认证令牌管理 - 需要创建
3. 数据库配置管理 - 需要创建
4. 配置文件管理 - 需要创建

### 第二阶段（本周）
5. 网络超时和重试配置
6. Tauri 命令和浏览器 API 统一
7. 环境变量管理
8. 跨平台测试

### 第三阶段（本月）
9. 依赖版本管理
10. 日志管理
11. 测试环境隔离
12. 权限管理

---

## 相关文件

- `ENVIRONMENT_CONSISTENCY_STRATEGY.md` - 环境一致性策略
- `ENVIRONMENT_INCONSISTENCY_ANALYSIS.md` - 详细分析
- `SSE_TESTING_BEST_PRACTICES.md` - SSE 测试最佳实践
- `desktop-client/src/platform_utils.rs` - 跨平台工具库
- `desktop-client/src/environment_checker.rs` - 环境检查工具

---

## 下一步行动

1. **立即**：
   - [ ] 创建 `auth_token_manager.rs`
   - [ ] 创建 `database_config.rs`
   - [ ] 创建 `config_manager.rs`

2. **本周**：
   - [ ] 实现网络配置管理
   - [ ] 统一 API 调用方式
   - [ ] 创建 `.env.example`

3. **本月**：
   - [ ] 完成跨平台测试
   - [ ] 实现依赖版本检查
   - [ ] 完成日志管理

---

## 参考资源

- [Rust 跨平台开发](https://doc.rust-lang.org/book/ch19-06-macros.html)
- [Tauri 文档](https://tauri.app/docs/)
- [dirs crate](https://docs.rs/dirs/latest/dirs/)
- [环境变量管理](https://docs.rs/dotenv/latest/dotenv/)
- [配置管理](https://docs.rs/config/latest/config/)
