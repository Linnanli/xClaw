# 技术设计文档 - 企业级 AI Agent 平台

## 概述 (Overview)

### 系统简介

企业级 AI Agent 平台是一个基于安全微内核架构的政企级 AI 应用解决方案，旨在为政企客户提供安全、可控、可审计的 AI Agent 执行环境。系统通过三层架构设计，实现了从管控中心到终端执行的全链路安全保障。

### 核心价值

- **合规性**：支持国密算法栈（SM2/SM3/SM4），满足政企客户的合规要求
- **安全性**：基于 WASM 沙箱隔离、DLP 脱敏、数字签名验证的多层安全防护
- **可控性**：敏感操作物理拦截、人工审批、细粒度权限控制
- **可审计性**：全链路审计日志、离线暂存、摘要验证、防篡改存证
- **离线能力**：支持离线认证、本地推理、断点续传，适应内网环境

### 系统边界

**包含范围**：
- 管理后台的插件审核、策略分发、审计中心
- 桌面客户端的身份认证、对话交互、安全内核
- 执行环境的 WASM 沙箱、MCP 网桥、本地推理

**不包含范围**：
- 大模型训练和微调
- 移动端应用（iOS/Android）
- 公有云 SaaS 部署模式

### 技术栈概览

| 层级 | 技术选型 | 说明 |
|------|---------|------|
| 前端 UI | Vue 3 + TypeScript + Vite | 响应式组件化开发 |
| 桌面框架 | Tauri 2.x | Rust 后端 + Web 前端混合架构 |
| 后端核心 | Rust (Tokio + Axum) | 异步运行时 + Web 框架 |
| 数据库 | PostgreSQL + libSQL | 服务端关系型 + 客户端嵌入式 |
| WASM 运行时 | Wasmtime | 支持 WASI 和资源限制 |
| 国密算法 | GMSSL/Tassl (C FFI) | 通过 Rust FFI 调用 |
| 审计存储 | ClickHouse / TimescaleDB | 时序数据库，高吞吐写入 |

---

## 系统架构 (Architecture)

### 三层架构设计

系统采用三层架构，实现管控、执行、审计的职责分离：

```
┌─────────────────────────────────────────────────────────────┐
│                    管理后台 (Admin Backend)                   │
│                      内网部署 - 管控中心                       │
├─────────────────────────────────────────────────────────────┤
│  • Skills 安全审核流水线 (SCA + AST + 国密签名)               │
│  • 企业 Skills 商店 (版本管理 + 灰度发布)                     │
│  • 身份管理 (IAM: JWT + Session)                             │
│  • 策略引擎 (DLP 词库 + 敏感操作规则)                         │
│  • 行为审计中心 (日志同步 + 违规检测)                         │
└─────────────────────────────────────────────────────────────┘
                              ↕ HTTPS (TLS 1.3)
┌─────────────────────────────────────────────────────────────┐
│                  桌面客户端 (Desktop Client)                  │
│                    Tauri App - 用户终端                       │
├─────────────────────────────────────────────────────────────┤
│  UI 层:                                                       │
│    • 身份认证界面 (主密码 / SSO)                              │
│    • 对话渲染 (CoT 展示 + 动态水印)                           │
│    • 人工审批弹窗 (敏感操作拦截)                              │
│                                                               │
│  安全内核 (Security Kernel):                                 │
│    • 验签器 (SM2 公钥校验)                                    │
│    • DLP 脱敏引擎 (实时过滤)                                  │
│    • WASM 沙箱 (资源隔离)                                     │
│    • 离线审计代理 (加密存证)                                  │
│    • 同步管理器 (断点续传)                                    │
│                                                               │
│  本地存储: libSQL (SM4 全库加密)                              │
└─────────────────────────────────────────────────────────────┘
                              ↕ MCP Protocol
┌─────────────────────────────────────────────────────────────┐
│                   执行环境 (Execution Env)                    │
│                    隔离沙箱 - 插件运行                        │
├─────────────────────────────────────────────────────────────┤
│  • WASM 插件运行时 (Wasmtime)                                │
│  • MCP 协议网桥 (脚本插件通信)                                │
│  • ClawHub 插件市场 (社区生态)                                │
│  • 内网推理网关 (本地 LLM: DeepSeek/Qwen)                    │
└─────────────────────────────────────────────────────────────┘
```

### 核心交互流程

#### 1. 插件上架流程

```
开发者提交 Skill
    ↓
[SCA 供应链扫描] → 检测依赖漏洞
    ↓
[AST 静态审计] → 检测安全风险
    ↓
[国密签名机] → SM2 私钥签名
    ↓
[企业 Skills 商店] → 版本管理 + 灰度发布
    ↓
[桌面客户端] → 下载已验签插件
```

#### 2. 用户认证与策略同步

```
用户启动应用
    ↓
[输入主密码] → 解锁本地数据库
    ↓
[IAM 验证] → 颁发 JWT (1h) + Refresh Token (7d)
    ↓
[策略引擎] → 同步 DLP 词库 + 敏感操作规则
    ↓
[本地缓存] → 支持离线工作
```

#### 3. 安全推理流程

```
用户输入指令
    ↓
[DLP 脱敏引擎] → 本地实时过滤敏感信息
    ↓
[WASM 沙箱] → 加载已验签插件
    ↓
[推理网关] → 路由到本地 LLM 或外部 API
    ↓
[响应渲染] → 展示 CoT + 动态水印
    ↓
[审计代理] → 记录操作日志 (SM3 摘要)
```

#### 4. 敏感操作审批

```
AI Agent 尝试执行敏感操作 (如删除文件)
    ↓
[安全内核拦截] → 暂停执行
    ↓
[人工审批弹窗] → 展示操作详情 + 风险等级
    ↓
用户确认/拒绝
    ↓
[审计日志] → 记录审批结果
    ↓
继续执行 / 取消操作
```

#### 5. 离线审计与同步

```
离线状态下的操作
    ↓
[审计代理] → 加密存储到本地 (SM4)
    ↓
[SM3 摘要] → 防篡改签名
    ↓
恢复网络连接
    ↓
[同步管理器] → 断点续传上报
    ↓
[审计中心] → 验证摘要 + 存证
```

### 部署架构

#### 内网部署模式

```
┌─────────────────────────────────────────────────────────────┐
│                        企业内网环境                           │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌─────────────────┐         ┌─────────────────┐            │
│  │  管理后台集群    │         │  审计数据库      │            │
│  │  (Rust + Axum)  │◄────────┤  (ClickHouse)   │            │
│  └────────┬────────┘         └─────────────────┘            │
│           │                                                   │
│           │ HTTPS (TLS 1.3 + 双向认证)                       │
│           │                                                   │
│  ┌────────▼────────┐         ┌─────────────────┐            │
│  │  推理网关        │◄────────┤  本地 LLM       │            │
│  │  (Model Proxy)  │         │  (DeepSeek)     │            │
│  └────────┬────────┘         └─────────────────┘            │
│           │                                                   │
│           │ HTTPS                                             │
│           │                                                   │
│  ┌────────▼────────────────────────────────────┐            │
│  │         桌面客户端 (多台终端)                │            │
│  │  • 用户 A (Windows)                          │            │
│  │  • 用户 B (macOS)                            │            │
│  │  • 用户 C (Linux)                            │            │
│  └─────────────────────────────────────────────┘            │
│                                                               │
└─────────────────────────────────────────────────────────────┘
```

#### 离线工作模式

```
┌─────────────────────────────────────────┐
│         桌面客户端 (离线状态)            │
├─────────────────────────────────────────┤
│  • 主密码认证 (本地验证)                │
│  • 已下载插件 (离线验签)                │
│  • 本地 DLP 词库 (缓存)                 │
│  • 本地 LLM 推理                        │
│  • 审计日志暂存 (加密)                  │
│                                          │
│  ❌ 不可用功能:                         │
│    - 插件下载/更新                      │
│    - 策略同步                           │
│    - 外网 API 调用                      │
└─────────────────────────────────────────┘
         │
         │ 恢复网络连接
         ▼
┌─────────────────────────────────────────┐
│         自动同步流程                     │
├─────────────────────────────────────────┤
│  1. 检查策略版本 → 同步最新策略         │
│  2. 上报离线审计日志 (断点续传)         │
│  3. 检查插件更新 → 提示用户升级         │
└─────────────────────────────────────────┘
```

### 安全边界

#### 信任边界划分

```
┌─────────────────────────────────────────────────────────────┐
│                        信任域 1: 管理后台                     │
│  • 完全可信                                                   │
│  • 拥有 SM2 私钥                                              │
│  • 负责签名和策略分发                                         │
└─────────────────────────────────────────────────────────────┘
                              ↕
┌─────────────────────────────────────────────────────────────┐
│                        信任域 2: 桌面客户端                   │
│  • 半可信 (用户可能被攻击)                                    │
│  • 拥有 SM2 公钥 (验签)                                       │
│  • 本地数据加密存储                                           │
└─────────────────────────────────────────────────────────────┘
                              ↕
┌─────────────────────────────────────────────────────────────┐
│                        信任域 3: 执行环境                     │
│  • 不可信 (插件可能恶意)                                      │
│  • WASM 沙箱隔离                                              │
│  • 资源访问受限                                               │
└─────────────────────────────────────────────────────────────┘
```

---

## 核心组件设计 (Components and Interfaces)

### 1. 安全内核 (Security Kernel)

安全内核是桌面客户端的核心模块，负责所有安全相关的功能。

#### 1.1 加密提供者 (CryptoProvider)

**职责**: 抽象加密算法实现，支持国密/默认算法栈切换

**接口设计**:

```rust
/// 加密算法提供者 Trait
pub trait CryptoProvider: Send + Sync {
    /// 非对称签名验证
    fn verify_signature(&self, public_key: &[u8], message: &[u8], signature: &[u8]) -> Result<bool>;
    
    /// 对称加密
    fn encrypt(&self, key: &[u8], plaintext: &[u8]) -> Result<Vec<u8>>;
    
    /// 对称解密
    fn decrypt(&self, key: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>>;
    
    /// 哈希摘要
    fn hash(&self, data: &[u8]) -> Result<Vec<u8>>;
    
    /// 密钥派生
    fn derive_key(&self, password: &[u8], salt: &[u8], iterations: u32) -> Result<Vec<u8>>;
}

/// 默认算法栈实现
pub struct DefaultCryptoProvider {
    // AES-256-GCM, Ed25519, SHA256
}

/// 国密算法栈实现
pub struct GMCryptoProvider {
    // SM4, SM2, SM3
    gmssl_ctx: *mut c_void, // FFI 上下文
}
```

**配置切换**:

```toml
# config.toml
[crypto]
standard = "Default"  # 或 "ChinaCrypto"
```

**实现要点**:
- 使用 Rust FFI 调用 GMSSL/Tassl C 库
- 统一的错误处理和日志记录
- 性能优化：缓存密钥派生结果

#### 1.2 DLP 脱敏引擎 (DLPEngine)

**职责**: 实时检测和过滤敏感信息

**数据结构**:

```rust
/// DLP 规则
pub struct DLPRule {
    pub id: String,
    pub name: String,
    pub pattern: Regex,           // 正则表达式
    pub replacement: String,      // 脱敏替换文本
    pub severity: Severity,       // 风险等级
}

pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

/// DLP 引擎
pub struct DLPEngine {
    rules: Arc<RwLock<Vec<DLPRule>>>,
    cache: LruCache<String, String>,  // 脱敏结果缓存
}

impl DLPEngine {
    /// 同步 DLP 词库
    pub async fn sync_rules(&self, backend_url: &str) -> Result<()>;
    
    /// 脱敏文本
    pub fn sanitize(&self, text: &str) -> (String, Vec<DLPMatch>);
    
    /// 批量脱敏
    pub fn sanitize_batch(&self, texts: &[String]) -> Vec<(String, Vec<DLPMatch>)>;
}

/// 脱敏匹配结果
pub struct DLPMatch {
    pub rule_id: String,
    pub start: usize,
    pub end: usize,
    pub original: String,
    pub severity: Severity,
}
```

**内置规则示例**:

```rust
// 身份证号
DLPRule {
    id: "id_card_cn",
    pattern: r"\d{17}[\dXx]",
    replacement: "***************",
    severity: Severity::High,
}

// 手机号
DLPRule {
    id: "phone_cn",
    pattern: r"1[3-9]\d{9}",
    replacement: "***********",
    severity: Severity::Medium,
}

// 银行卡号
DLPRule {
    id: "bank_card",
    pattern: r"\d{16,19}",
    replacement: "****************",
    severity: Severity::Critical,
}
```

#### 1.3 WASM 沙箱 (WASMSandbox)

**职责**: 隔离执行 WASM 插件，限制资源访问

**架构设计**:

```rust
use wasmtime::*;

/// WASM 沙箱配置
pub struct SandboxConfig {
    pub max_memory: usize,        // 最大内存 (字节)
    pub max_execution_time: Duration,  // 最大执行时间
    pub allowed_hosts: Vec<String>,    // 允许访问的主机
    pub allowed_paths: Vec<PathBuf>,   // 允许访问的路径
}

/// WASM 沙箱
pub struct WASMSandbox {
    engine: Engine,
    linker: Linker<SandboxState>,
    config: SandboxConfig,
}

/// 沙箱状态
pub struct SandboxState {
    pub audit_log: Arc<Mutex<AuditLog>>,
    pub dlp_engine: Arc<DLPEngine>,
    pub approval_tx: mpsc::Sender<ApprovalRequest>,
}

impl WASMSandbox {
    /// 加载并验证插件
    pub async fn load_plugin(&self, plugin_path: &Path, signature: &[u8]) -> Result<Plugin>;
    
    /// 执行插件函数
    pub async fn execute(&self, plugin: &Plugin, function: &str, args: &[Value]) -> Result<Vec<Value>>;
    
    /// 导出宿主函数给 WASM
    fn register_host_functions(&mut self) -> Result<()>;
}
```

**宿主函数导出**:

```rust
// 文件读取 (受限)
linker.func_wrap("env", "read_file", |caller: Caller<'_, SandboxState>, path_ptr: i32, path_len: i32| -> i32 {
    let memory = caller.get_export("memory").unwrap().into_memory().unwrap();
    let path = read_string_from_memory(&memory, path_ptr, path_len)?;
    
    // 检查路径是否在允许列表中
    if !caller.data().is_path_allowed(&path) {
        // 触发审批流程
        let approval = caller.data().request_approval(ApprovalRequest {
            operation: "read_file",
            target: path.clone(),
            risk_level: RiskLevel::Medium,
        }).await?;
        
        if !approval.approved {
            return Err(Error::PermissionDenied);
        }
    }
    
    // 读取文件并返回
    let content = std::fs::read_to_string(&path)?;
    write_string_to_memory(&memory, &content)
});

// 网络请求 (受限)
linker.func_wrap("env", "http_request", |caller: Caller<'_, SandboxState>, url_ptr: i32, url_len: i32| -> i32 {
    let url = read_string_from_memory(&memory, url_ptr, url_len)?;
    
    // 检查是否为外网访问
    if is_external_url(&url) {
        // 强制审批
        let approval = caller.data().request_approval(ApprovalRequest {
            operation: "http_request",
            target: url.clone(),
            risk_level: RiskLevel::High,
        }).await?;
        
        if !approval.approved {
            return Err(Error::PermissionDenied);
        }
    }
    
    // 执行请求
    let response = reqwest::get(&url).await?;
    write_bytes_to_memory(&memory, &response.bytes().await?)
});
```

#### 1.4 审计代理 (AuditProxy)

**职责**: 记录操作日志，支持离线暂存和在线同步

**数据结构**:

```rust
/// 审计日志条目
#[derive(Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub user_id: String,
    pub session_id: String,
    pub operation: String,
    pub target: Option<String>,
    pub result: OperationResult,
    pub metadata: serde_json::Value,
    pub digest: Vec<u8>,  // SM3 或 SHA256 摘要
}

pub enum OperationResult {
    Success,
    Failed(String),
    Denied,
}

/// 审计代理
pub struct AuditProxy {
    local_store: Arc<Mutex<LocalAuditStore>>,
    sync_manager: Arc<SyncManager>,
    crypto: Arc<dyn CryptoProvider>,
}

impl AuditProxy {
    /// 记录日志
    pub async fn log(&self, entry: AuditLogEntry) -> Result<()> {
        // 计算摘要
        let digest = self.crypto.hash(&serde_json::to_vec(&entry)?)?;
        let entry = AuditLogEntry { digest, ..entry };
        
        // 加密存储到本地
        self.local_store.lock().await.insert(entry.clone())?;
        
        // 尝试同步到后端
        if self.is_online().await {
            self.sync_manager.sync_log(entry).await?;
        }
        
        Ok(())
    }
    
    /// 批量同步离线日志
    pub async fn sync_offline_logs(&self) -> Result<SyncResult>;
}
```

### 2. MCP 协议网桥 (MCP Bridge)

**职责**: 支持脚本插件通过 MCP 协议与宿主通信，并在沙箱环境中隔离执行

**架构设计**:

```rust
/// MCP 网桥
pub struct MCPBridge {
    servers: HashMap<String, MCPServerHandle>,
    sandbox_manager: Arc<SandboxManager>,
    audit_log: Arc<AuditProxy>,
}

/// MCP 服务器句柄
pub struct MCPServerHandle {
    pub name: String,
    pub sandbox_id: String,
    pub process: Child,
    pub stdin: ChildStdin,
    pub stdout: BufReader<ChildStdout>,
}

impl MCPBridge {
    /// 启动 MCP 服务器 (Python/Node.js 脚本) 在沙箱中
    pub async fn start_server(&mut self, script_path: &Path, runtime: Runtime, sandbox_config: SandboxConfig) -> Result<String>;
    
    /// 调用工具
    pub async fn call_tool(&self, server_name: &str, tool_name: &str, args: serde_json::Value) -> Result<serde_json::Value>;
    
    /// 停止服务器并清理沙箱
    pub async fn stop_server(&mut self, server_name: &str) -> Result<()>;
}

pub enum Runtime {
    Python,
    NodeJS,
    Ruby,
}
```

#### 2.1 脚本沙箱技术选型

**需求**:
- 跨平台支持 (Windows/macOS/Linux)
- 安全隔离 (文件系统、网络、进程)
- 性能开销可接受
- 易于集成和管理

**技术选型对比**:

| 方案 | Windows | macOS | Linux | 安全性 | 性能 | 复杂度 | 推荐度 |
|------|---------|-------|-------|--------|------|--------|--------|
| **Bubblewrap** | ❌ | ❌ | ✅ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | ❌ |
| **Firejail** | ❌ | ❌ | ✅ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐ | ❌ |
| **gVisor** | ✅ | ❌ | ✅ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐ | ❌ |
| **Docker** | ✅ | ✅ | ✅ | ⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐ | ⚠️ |
| **Podman** | ✅ | ✅ | ✅ | ⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐ | ⚠️ |
| **Landlock (Linux)** | ❌ | ❌ | ✅ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | ❌ |
| **Sandbox-exec (macOS)** | ❌ | ✅ | ❌ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | ❌ |
| **AppContainer (Windows)** | ✅ | ❌ | ❌ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ❌ |
| **Deno Runtime** | ✅ | ✅ | ✅ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐ | ✅ |
| **自定义混合方案** | ✅ | ✅ | ✅ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ✅✅ |

**最终决策**: 采用**自定义混合沙箱方案**

**理由**:
- **跨平台统一**: 在不同操作系统上使用各自的原生沙箱技术
- **安全性**: 利用操作系统级别的隔离机制
- **性能**: 避免容器化的额外开销
- **灵活性**: 可根据平台特性优化

**实现方案**:

```rust
/// 沙箱管理器 - 跨平台抽象
pub struct SandboxManager {
    #[cfg(target_os = "linux")]
    linux_sandbox: LinuxSandbox,
    
    #[cfg(target_os = "macos")]
    macos_sandbox: MacOSSandbox,
    
    #[cfg(target_os = "windows")]
    windows_sandbox: WindowsSandbox,
}

/// 沙箱配置
pub struct SandboxConfig {
    pub allowed_paths: Vec<PathBuf>,      // 允许访问的路径
    pub allowed_network: bool,             // 是否允许网络访问
    pub allowed_hosts: Vec<String>,        // 允许访问的主机列表
    pub max_memory: usize,                 // 最大内存限制 (字节)
    pub max_cpu_percent: u8,               // CPU 使用率限制 (%)
    pub max_processes: u32,                // 最大进程数
    pub timeout: Duration,                 // 执行超时时间
    pub env_vars: HashMap<String, String>, // 环境变量
}

impl SandboxManager {
    /// 创建沙箱并启动进程
    pub async fn spawn(&self, command: &str, args: &[String], config: SandboxConfig) -> Result<SandboxProcess>;
    
    /// 终止沙箱进程
    pub async fn terminate(&self, sandbox_id: &str) -> Result<()>;
    
    /// 获取沙箱资源使用情况
    pub async fn get_stats(&self, sandbox_id: &str) -> Result<SandboxStats>;
}

/// Linux 沙箱实现 (使用 Landlock + Seccomp + Namespaces)
#[cfg(target_os = "linux")]
pub struct LinuxSandbox {
    // 使用 Linux 内核特性：
    // - Landlock LSM: 文件系统访问控制
    // - Seccomp-BPF: 系统调用过滤
    // - Namespaces: 进程、网络、挂载点隔离
    // - Cgroups v2: 资源限制
}

#[cfg(target_os = "linux")]
impl LinuxSandbox {
    fn apply_landlock_rules(&self, config: &SandboxConfig) -> Result<()> {
        // 使用 landlock crate 限制文件系统访问
        use landlock::*;
        
        let mut ruleset = Ruleset::new()
            .handle_access(AccessFs::ReadFile)?
            .handle_access(AccessFs::ReadDir)?;
        
        for path in &config.allowed_paths {
            ruleset = ruleset.add_rule(PathBeneath::new(path, AccessFs::ReadFile | AccessFs::ReadDir))?;
        }
        
        ruleset.restrict_self()?;
        Ok(())
    }
    
    fn apply_seccomp_filter(&self) -> Result<()> {
        // 使用 seccomp crate 限制系统调用
        use seccomp::*;
        
        let mut filter = SeccompFilter::new(Action::Errno(libc::EPERM))?;
        // 允许基本系统调用
        filter.add_rule(Action::Allow, Syscall::read)?;
        filter.add_rule(Action::Allow, Syscall::write)?;
        filter.add_rule(Action::Allow, Syscall::exit)?;
        // ... 其他必要的系统调用
        
        filter.load()?;
        Ok(())
    }
    
    fn setup_namespaces(&self) -> Result<()> {
        // 使用 nix crate 创建命名空间
        use nix::sched::*;
        
        unshare(CloneFlags::CLONE_NEWPID | CloneFlags::CLONE_NEWNET | CloneFlags::CLONE_NEWNS)?;
        Ok(())
    }
    
    fn apply_cgroup_limits(&self, config: &SandboxConfig) -> Result<()> {
        // 使用 cgroups-rs 限制资源
        use cgroups_rs::*;
        
        let cg = Cgroup::new(hierarchies::auto(), "ironclaw_sandbox")?;
        
        // 内存限制
        let mem_controller = cg.controller_of::<MemController>().unwrap();
        mem_controller.set_limit(config.max_memory as i64)?;
        
        // CPU 限制
        let cpu_controller = cg.controller_of::<CpuController>().unwrap();
        cpu_controller.set_shares(config.max_cpu_percent as u64 * 10)?;
        
        Ok(())
    }
}

/// macOS 沙箱实现 (使用 sandbox-exec + App Sandbox)
#[cfg(target_os = "macos")]
pub struct MacOSSandbox {
    // 使用 macOS 沙箱特性：
    // - sandbox-exec: 沙箱配置文件
    // - App Sandbox: 应用级隔离
    // - Endpoint Security: 进程监控
}

#[cfg(target_os = "macos")]
impl MacOSSandbox {
    fn generate_sandbox_profile(&self, config: &SandboxConfig) -> String {
        // 生成 sandbox-exec 配置文件
        let mut profile = String::from("(version 1)\n");
        profile.push_str("(deny default)\n");
        
        // 允许基本操作
        profile.push_str("(allow process-exec)\n");
        profile.push_str("(allow sysctl-read)\n");
        
        // 文件系统访问
        for path in &config.allowed_paths {
            profile.push_str(&format!(
                "(allow file-read* file-write* (subpath \"{}\"))\n",
                path.display()
            ));
        }
        
        // 网络访问
        if config.allowed_network {
            profile.push_str("(allow network-outbound)\n");
            for host in &config.allowed_hosts {
                profile.push_str(&format!(
                    "(allow network-outbound (remote ip \"{}:*\"))\n",
                    host
                ));
            }
        } else {
            profile.push_str("(deny network*)\n");
        }
        
        profile
    }
    
    fn spawn_with_sandbox(&self, command: &str, profile: &str) -> Result<Child> {
        // 使用 sandbox-exec 启动进程
        Command::new("sandbox-exec")
            .arg("-p")
            .arg(profile)
            .arg(command)
            .spawn()
            .map_err(Into::into)
    }
}

/// Windows 沙箱实现 (使用 Job Objects + AppContainer)
#[cfg(target_os = "windows")]
pub struct WindowsSandbox {
    // 使用 Windows 沙箱特性：
    // - Job Objects: 进程组管理和资源限制
    // - AppContainer: 应用隔离
    // - Integrity Levels: 访问控制
}

#[cfg(target_os = "windows")]
impl WindowsSandbox {
    fn create_job_object(&self, config: &SandboxConfig) -> Result<HANDLE> {
        use winapi::um::jobapi2::*;
        use winapi::um::winnt::*;
        
        unsafe {
            let job = CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
            
            // 设置资源限制
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = 
                JOB_OBJECT_LIMIT_PROCESS_MEMORY | 
                JOB_OBJECT_LIMIT_PROCESS_TIME |
                JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
            
            limits.ProcessMemoryLimit = config.max_memory;
            limits.BasicLimitInformation.ActiveProcessLimit = config.max_processes;
            
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &mut limits as *mut _ as *mut _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            
            Ok(job)
        }
    }
    
    fn create_appcontainer(&self, config: &SandboxConfig) -> Result<AppContainerProfile> {
        // 创建 AppContainer 配置
        // 限制文件系统、网络、注册表访问
        unimplemented!("AppContainer implementation")
    }
}
```

**沙箱安全特性对比**:

| 特性 | Linux | macOS | Windows |
|------|-------|-------|---------|
| 文件系统隔离 | Landlock + Namespaces | sandbox-exec | AppContainer |
| 网络隔离 | Network Namespaces | sandbox-exec | AppContainer |
| 进程隔离 | PID Namespaces | sandbox-exec | Job Objects |
| 系统调用过滤 | Seccomp-BPF | sandbox-exec | - |
| 资源限制 | Cgroups v2 | launchd limits | Job Objects |
| 性能开销 | 极低 (~1%) | 低 (~2-3%) | 低 (~2-3%) |

**备选方案**: 如果需要更简单的实现，可以考虑使用 **Deno Runtime** 作为脚本执行环境：

```rust
/// Deno 沙箱实现 (跨平台统一方案)
pub struct DenoSandbox {
    deno_path: PathBuf,
}

impl DenoSandbox {
    pub async fn run_script(&self, script_path: &Path, config: &SandboxConfig) -> Result<Output> {
        let mut cmd = Command::new(&self.deno_path);
        cmd.arg("run");
        
        // 权限控制
        if config.allowed_network {
            cmd.arg("--allow-net");
            for host in &config.allowed_hosts {
                cmd.arg(format!("--allow-net={}", host));
            }
        }
        
        for path in &config.allowed_paths {
            cmd.arg(format!("--allow-read={}", path.display()));
            cmd.arg(format!("--allow-write={}", path.display()));
        }
        
        // 资源限制
        cmd.env("DENO_MEMORY_LIMIT", config.max_memory.to_string());
        
        cmd.arg(script_path);
        cmd.output().await.map_err(Into::into)
    }
}
```

**Deno 方案优势**:
- 跨平台统一实现
- 内置权限系统
- 性能优秀
- 支持 TypeScript/JavaScript

**Deno 方案劣势**:
- 仅支持 JS/TS，不支持 Python
- 需要额外安装 Deno 运行时
- 权限粒度相对较粗

**MCP 协议示例**:

```json
// 请求
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "search_web",
    "arguments": {
      "query": "Rust async programming"
    }
  }
}

// 响应
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Found 10 results..."
      }
    ]
  }
}
```

### 3. 推理网关 (LLM Gateway)

**职责**: 统一模型请求出口，支持本地/远程 LLM

**架构设计**:

```rust
/// 推理网关
pub struct LLMGateway {
    router: ModelRouter,
    dlp_engine: Arc<DLPEngine>,
    audit_log: Arc<AuditProxy>,
}

/// 模型路由器
pub struct ModelRouter {
    local_models: HashMap<String, LocalModel>,
    remote_endpoints: HashMap<String, RemoteEndpoint>,
}

pub struct LocalModel {
    pub name: String,
    pub path: PathBuf,
    pub context_length: usize,
}

pub struct RemoteEndpoint {
    pub name: String,
    pub url: String,
    pub api_key: Option<String>,
}

impl LLMGateway {
    /// 推理请求
    pub async fn infer(&self, request: InferenceRequest) -> Result<InferenceResponse> {
        // 1. DLP 脱敏
        let (sanitized_prompt, matches) = self.dlp_engine.sanitize(&request.prompt);
        
        // 2. 记录审计日志
        self.audit_log.log(AuditLogEntry {
            operation: "llm_inference",
            metadata: json!({
                "model": request.model,
                "dlp_matches": matches.len(),
            }),
            ..Default::default()
        }).await?;
        
        // 3. 路由到模型
        let response = if self.is_offline().await {
            self.router.infer_local(&request.model, &sanitized_prompt).await?
        } else {
            self.router.infer_remote(&request.model, &sanitized_prompt).await?
        };
        
        Ok(response)
    }
}
```

### 4. 同步管理器 (SyncManager)

**职责**: 管理离线数据的断点续传同步

**实现设计**:

```rust
/// 同步管理器
pub struct SyncManager {
    backend_url: String,
    local_store: Arc<Mutex<LocalAuditStore>>,
    sync_state: Arc<RwLock<SyncState>>,
}

/// 同步状态
pub struct SyncState {
    pub last_sync_time: DateTime<Utc>,
    pub pending_logs: usize,
    pub syncing: bool,
}

impl SyncManager {
    /// 启动自动同步
    pub async fn start_auto_sync(&self, interval: Duration) {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(interval).await;
                if self.is_online().await {
                    let _ = self.sync_all().await;
                }
            }
        });
    }
    
    /// 同步所有待上报日志
    pub async fn sync_all(&self) -> Result<SyncResult> {
        let mut state = self.sync_state.write().await;
        if state.syncing {
            return Ok(SyncResult::AlreadySyncing);
        }
        state.syncing = true;
        drop(state);
        
        let logs = self.local_store.lock().await.get_unsynced()?;
        let mut synced = 0;
        
        for log in logs {
            match self.upload_log(&log).await {
                Ok(_) => {
                    self.local_store.lock().await.mark_synced(log.id)?;
                    synced += 1;
                }
                Err(e) => {
                    // 断点续传：记录失败位置
                    log::error!("Failed to sync log {}: {}", log.id, e);
                    break;
                }
            }
        }
        
        let mut state = self.sync_state.write().await;
        state.syncing = false;
        state.last_sync_time = Utc::now();
        state.pending_logs -= synced;
        
        Ok(SyncResult::Success { synced })
    }
}
```

---

## 数据模型 (Data Models)



### 管理后台数据模型 (PostgreSQL)

#### 1. 用户表 (users)

```sql
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username VARCHAR(255) UNIQUE NOT NULL,
    email VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255),  -- MVP: 可为空 (主密码模式)
    department VARCHAR(255),
    role VARCHAR(50) NOT NULL,   -- admin, user, auditor
    status VARCHAR(20) NOT NULL DEFAULT 'active',  -- active, disabled
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_users_username ON users(username);
CREATE INDEX idx_users_email ON users(email);
```

#### 2. 插件表 (skills)

```sql
CREATE TABLE skills (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) UNIQUE NOT NULL,
    display_name VARCHAR(255) NOT NULL,
    description TEXT,
    author VARCHAR(255),
    source VARCHAR(50) NOT NULL,  -- internal, clawhub
    skill_type VARCHAR(50) NOT NULL,  -- wasm, python, nodejs
    current_version_id UUID,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',  -- pending, approved, rejected
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    FOREIGN KEY (current_version_id) REFERENCES skill_versions(id)
);

CREATE INDEX idx_skills_name ON skills(name);
CREATE INDEX idx_skills_source ON skills(source);
CREATE INDEX idx_skills_status ON skills(status);
```

#### 3. 插件版本表 (skill_versions)

```sql
CREATE TABLE skill_versions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    skill_id UUID NOT NULL,
    version VARCHAR(50) NOT NULL,
    changelog TEXT,
    binary_hash VARCHAR(64) NOT NULL,  -- SHA256 哈希
    signature BYTEA NOT NULL,  -- SM2 签名
    file_path VARCHAR(500) NOT NULL,
    file_size BIGINT NOT NULL,
    audit_status VARCHAR(20) NOT NULL DEFAULT 'pending',  -- pending, passed, failed
    audit_report JSONB,
    published_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    FOREIGN KEY (skill_id) REFERENCES skills(id) ON DELETE CASCADE,
    UNIQUE(skill_id, version)
);

CREATE INDEX idx_skill_versions_skill_id ON skill_versions(skill_id);
CREATE INDEX idx_skill_versions_audit_status ON skill_versions(audit_status);
```

#### 4. 灰度发布表 (canary_deployments)

```sql
CREATE TABLE canary_deployments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    skill_version_id UUID NOT NULL,
    stage VARCHAR(20) NOT NULL,  -- alpha, beta, ga
    target_groups JSONB NOT NULL,  -- ["group1", "group2"]
    rollout_percentage INTEGER NOT NULL,  -- 5, 20, 100
    error_threshold DECIMAL(5,2) NOT NULL DEFAULT 5.0,  -- 错误率阈值 (%)
    current_error_rate DECIMAL(5,2) DEFAULT 0.0,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',  -- pending, running, paused, completed, rolled_back
    started_at TIMESTAMP WITH TIME ZONE,
    completed_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    FOREIGN KEY (skill_version_id) REFERENCES skill_versions(id) ON DELETE CASCADE
);

CREATE INDEX idx_canary_deployments_status ON canary_deployments(status);
```

#### 5. DLP 规则表 (dlp_rules)

```sql
CREATE TABLE dlp_rules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) UNIQUE NOT NULL,
    pattern TEXT NOT NULL,  -- 正则表达式
    replacement VARCHAR(255) NOT NULL,
    severity VARCHAR(20) NOT NULL,  -- low, medium, high, critical
    enabled BOOLEAN NOT NULL DEFAULT true,
    version INTEGER NOT NULL DEFAULT 1,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_dlp_rules_enabled ON dlp_rules(enabled);
```

#### 6. 敏感操作规则表 (sensitive_operation_rules)

```sql
CREATE TABLE sensitive_operation_rules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    operation_type VARCHAR(100) NOT NULL,  -- file_delete, network_external, etc.
    risk_level VARCHAR(20) NOT NULL,  -- low, medium, high, critical
    require_approval BOOLEAN NOT NULL DEFAULT true,
    auto_deny BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_sensitive_operation_rules_operation_type ON sensitive_operation_rules(operation_type);
```

#### 7. 审计日志表 (audit_logs)

```sql
CREATE TABLE audit_logs (
    id UUID PRIMARY KEY,
    timestamp TIMESTAMP WITH TIME ZONE NOT NULL,
    user_id UUID NOT NULL,
    session_id UUID NOT NULL,
    operation VARCHAR(100) NOT NULL,
    target VARCHAR(500),
    result VARCHAR(20) NOT NULL,  -- success, failed, denied
    metadata JSONB,
    digest BYTEA NOT NULL,  -- SM3 或 SHA256 摘要
    client_ip INET,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    FOREIGN KEY (user_id) REFERENCES users(id)
);

-- 使用 TimescaleDB 时序扩展
SELECT create_hypertable('audit_logs', 'timestamp');

CREATE INDEX idx_audit_logs_user_id ON audit_logs(user_id);
CREATE INDEX idx_audit_logs_operation ON audit_logs(operation);
CREATE INDEX idx_audit_logs_timestamp ON audit_logs(timestamp DESC);
```

### 桌面客户端数据模型 (libSQL)

#### 1. 本地配置表 (local_config)

```sql
CREATE TABLE local_config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    encrypted BOOLEAN NOT NULL DEFAULT false,
    updated_at INTEGER NOT NULL  -- Unix timestamp
);
```

#### 2. 本地审计日志表 (local_audit_logs)

```sql
CREATE TABLE local_audit_logs (
    id TEXT PRIMARY KEY,  -- UUID
    timestamp INTEGER NOT NULL,  -- Unix timestamp
    user_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    operation TEXT NOT NULL,
    target TEXT,
    result TEXT NOT NULL,
    metadata TEXT,  -- JSON string
    digest BLOB NOT NULL,
    synced BOOLEAN NOT NULL DEFAULT false,
    created_at INTEGER NOT NULL
);

CREATE INDEX idx_local_audit_logs_synced ON local_audit_logs(synced);
CREATE INDEX idx_local_audit_logs_timestamp ON local_audit_logs(timestamp DESC);
```

#### 3. 缓存的 DLP 规则表 (cached_dlp_rules)

```sql
CREATE TABLE cached_dlp_rules (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    pattern TEXT NOT NULL,
    replacement TEXT NOT NULL,
    severity TEXT NOT NULL,
    enabled BOOLEAN NOT NULL,
    version INTEGER NOT NULL,
    synced_at INTEGER NOT NULL
);
```

#### 4. 已下载插件表 (downloaded_skills)

```sql
CREATE TABLE downloaded_skills (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    version TEXT NOT NULL,
    file_path TEXT NOT NULL,
    signature BLOB NOT NULL,
    downloaded_at INTEGER NOT NULL
);

CREATE INDEX idx_downloaded_skills_name ON downloaded_skills(name);
```

---

## 接口设计 (Interfaces)

### REST API (管理后台)

#### 1. 身份认证 API

**POST /api/auth/login**

请求:
```json
{
  "username": "user@example.com",
  "password": "********"
}
```

响应:
```json
{
  "access_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "refresh_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "expires_in": 3600,
  "user": {
    "id": "uuid",
    "username": "user@example.com",
    "role": "user"
  }
}
```

**POST /api/auth/refresh**

请求:
```json
{
  "refresh_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
}
```

响应:
```json
{
  "access_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "expires_in": 3600
}
```

#### 2. 插件管理 API

**GET /api/skills**

查询参数:
- `source`: internal | clawhub
- `status`: pending | approved | rejected
- `page`: 页码
- `limit`: 每页数量

响应:
```json
{
  "skills": [
    {
      "id": "uuid",
      "name": "web-search",
      "display_name": "网页搜索",
      "description": "搜索互联网内容",
      "author": "ClawHub Community",
      "source": "clawhub",
      "skill_type": "wasm",
      "current_version": "1.2.0",
      "status": "approved",
      "created_at": "2024-01-01T00:00:00Z"
    }
  ],
  "total": 100,
  "page": 1,
  "limit": 20
}
```

**POST /api/skills**

上传插件 (multipart/form-data):
- `file`: 插件二进制文件
- `metadata`: JSON 元数据

```json
{
  "name": "custom-tool",
  "display_name": "自定义工具",
  "description": "企业内部工具",
  "skill_type": "wasm",
  "version": "1.0.0"
}
```

响应:
```json
{
  "id": "uuid",
  "status": "pending",
  "message": "插件已提交，等待审核"
}
```

**GET /api/skills/{id}/versions**

响应:
```json
{
  "versions": [
    {
      "id": "uuid",
      "version": "1.2.0",
      "changelog": "修复安全漏洞",
      "audit_status": "passed",
      "published_at": "2024-01-15T00:00:00Z"
    },
    {
      "id": "uuid",
      "version": "1.1.0",
      "changelog": "新增功能",
      "audit_status": "passed",
      "published_at": "2024-01-01T00:00:00Z"
    }
  ]
}
```

**POST /api/skills/{id}/versions/{version_id}/deploy**

启动灰度发布:
```json
{
  "stages": [
    {
      "stage": "alpha",
      "target_groups": ["internal-test"],
      "rollout_percentage": 5,
      "error_threshold": 5.0
    },
    {
      "stage": "beta",
      "target_groups": ["early-adopters"],
      "rollout_percentage": 20,
      "error_threshold": 3.0
    },
    {
      "stage": "ga",
      "target_groups": ["all"],
      "rollout_percentage": 100,
      "error_threshold": 1.0
    }
  ]
}
```

响应:
```json
{
  "deployment_id": "uuid",
  "status": "running",
  "current_stage": "alpha"
}
```

#### 3. 策略管理 API

**GET /api/policies/dlp**

响应:
```json
{
  "rules": [
    {
      "id": "uuid",
      "name": "id_card_cn",
      "pattern": "\\d{17}[\\dXx]",
      "replacement": "***************",
      "severity": "high",
      "enabled": true,
      "version": 2
    }
  ],
  "version": 5
}
```

**POST /api/policies/dlp**

创建 DLP 规则:
```json
{
  "name": "custom_pattern",
  "pattern": "[A-Z]{3}-\\d{6}",
  "replacement": "***-******",
  "severity": "medium"
}
```

**GET /api/policies/sensitive-operations**

响应:
```json
{
  "rules": [
    {
      "id": "uuid",
      "operation_type": "file_delete",
      "risk_level": "high",
      "require_approval": true,
      "auto_deny": false
    },
    {
      "id": "uuid",
      "operation_type": "network_external",
      "risk_level": "critical",
      "require_approval": true,
      "auto_deny": false
    }
  ]
}
```

#### 4. 审计日志 API

**POST /api/audit/logs**

批量上报审计日志:
```json
{
  "logs": [
    {
      "id": "uuid",
      "timestamp": "2024-01-01T12:00:00Z",
      "user_id": "uuid",
      "session_id": "uuid",
      "operation": "file_read",
      "target": "/home/user/document.txt",
      "result": "success",
      "metadata": {},
      "digest": "base64-encoded-hash"
    }
  ]
}
```

响应:
```json
{
  "accepted": 100,
  "rejected": 0,
  "errors": []
}
```

**GET /api/audit/logs**

查询审计日志:

查询参数:
- `user_id`: 用户 ID
- `operation`: 操作类型
- `start_time`: 开始时间
- `end_time`: 结束时间
- `page`: 页码
- `limit`: 每页数量

响应:
```json
{
  "logs": [
    {
      "id": "uuid",
      "timestamp": "2024-01-01T12:00:00Z",
      "user_id": "uuid",
      "username": "user@example.com",
      "operation": "file_read",
      "target": "/home/user/document.txt",
      "result": "success",
      "client_ip": "192.168.1.100"
    }
  ],
  "total": 1000,
  "page": 1,
  "limit": 50
}
```

**GET /api/audit/violations**

查询违规行为:

响应:
```json
{
  "violations": [
    {
      "id": "uuid",
      "user_id": "uuid",
      "username": "user@example.com",
      "operation": "network_external",
      "target": "https://malicious-site.com",
      "detected_at": "2024-01-01T12:00:00Z",
      "severity": "critical",
      "status": "pending"
    }
  ]
}
```

### Tauri Commands (桌面客户端)

#### 1. 身份认证命令

```rust
#[tauri::command]
async fn unlock_with_password(password: String) -> Result<UnlockResult, String> {
    // 验证主密码
    // 解密本地数据库
    // 返回会话信息
}

#[tauri::command]
async fn lock_session() -> Result<(), String> {
    // 锁定会话
    // 清除内存中的敏感数据
}
```

#### 2. 插件管理命令

```rust
#[tauri::command]
async fn list_available_skills() -> Result<Vec<Skill>, String> {
    // 从管理后台获取可用插件列表
}

#[tauri::command]
async fn download_skill(skill_id: String, version: String) -> Result<DownloadProgress, String> {
    // 下载插件到本地
    // 验证签名
}

#[tauri::command]
async fn load_skill(skill_id: String) -> Result<(), String> {
    // 加载插件到 WASM 沙箱
}
```

#### 3. 对话交互命令

```rust
#[tauri::command]
async fn send_message(message: String) -> Result<StreamResponse, String> {
    // DLP 脱敏
    // 发送到 LLM
    // 流式返回响应
}

#[tauri::command]
async fn approve_sensitive_operation(request_id: String, approved: bool) -> Result<(), String> {
    // 处理人工审批
}
```

#### 4. 同步管理命令

```rust
#[tauri::command]
async fn sync_policies() -> Result<SyncResult, String> {
    // 同步 DLP 词库和敏感操作规则
}

#[tauri::command]
async fn sync_audit_logs() -> Result<SyncResult, String> {
    // 上报离线审计日志
}
```

### MCP 协议接口

#### 工具调用

请求:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "search_web",
    "arguments": {
      "query": "Rust async programming",
      "max_results": 10
    }
  }
}
```

响应:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Found 10 results:\n1. ..."
      }
    ]
  }
}
```

#### 工具列表

请求:
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/list"
}
```

响应:
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "tools": [
      {
        "name": "search_web",
        "description": "Search the web for information",
        "inputSchema": {
          "type": "object",
          "properties": {
            "query": {
              "type": "string",
              "description": "Search query"
            },
            "max_results": {
              "type": "integer",
              "description": "Maximum number of results",
              "default": 10
            }
          },
          "required": ["query"]
        }
      }
    ]
  }
}
```

---

## 错误处理 (Error Handling)

### 错误分类

系统定义了统一的错误分类体系：

```rust
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    // 认证错误
    #[error("认证失败: {0}")]
    AuthenticationFailed(String),
    
    #[error("会话已过期")]
    SessionExpired,
    
    #[error("权限不足")]
    PermissionDenied,
    
    // 加密错误
    #[error("签名验证失败: {0}")]
    SignatureVerificationFailed(String),
    
    #[error("加密失败: {0}")]
    EncryptionFailed(String),
    
    #[error("解密失败: {0}")]
    DecryptionFailed(String),
    
    // 插件错误
    #[error("插件加载失败: {0}")]
    PluginLoadFailed(String),
    
    #[error("插件执行超时")]
    PluginExecutionTimeout,
    
    #[error("插件资源超限: {0}")]
    PluginResourceExceeded(String),
    
    // 网络错误
    #[error("网络请求失败: {0}")]
    NetworkError(String),
    
    #[error("连接超时")]
    ConnectionTimeout,
    
    // 数据库错误
    #[error("数据库错误: {0}")]
    DatabaseError(String),
    
    // 审计错误
    #[error("审计日志写入失败: {0}")]
    AuditLogWriteFailed(String),
    
    #[error("审计日志摘要验证失败")]
    AuditLogDigestMismatch,
    
    // 业务错误
    #[error("DLP 检测到敏感信息")]
    DLPViolation,
    
    #[error("操作需要人工审批")]
    ApprovalRequired,
    
    #[error("操作被拒绝: {0}")]
    OperationDenied(String),
}
```

### 错误处理策略

#### 1. 认证错误

- **策略**: 立即锁定会话，要求重新认证
- **日志**: 记录失败尝试到审计日志
- **用户提示**: "认证失败，请重新登录"

#### 2. 签名验证失败

- **策略**: 拒绝加载插件，记录安全事件
- **日志**: 记录到审计日志，标记为高风险事件
- **用户提示**: "插件签名验证失败，可能已被篡改"
- **管理员通知**: 发送告警邮件

#### 3. DLP 违规

- **策略**: 阻止请求发送，脱敏后重试
- **日志**: 记录违规内容的哈希值（不记录原文）
- **用户提示**: "检测到敏感信息，已自动脱敏"

#### 4. 插件执行超时

- **策略**: 强制终止插件，释放资源
- **日志**: 记录超时事件和插件信息
- **用户提示**: "插件执行超时，已自动终止"

#### 5. 网络错误

- **策略**: 自动切换到离线模式
- **日志**: 记录网络状态变化
- **用户提示**: "网络连接失败，已切换到离线模式"

### 错误恢复机制

#### 1. 自动重试

```rust
pub async fn retry_with_backoff<F, T, E>(
    mut f: F,
    max_retries: u32,
    initial_delay: Duration,
) -> Result<T, E>
where
    F: FnMut() -> Pin<Box<dyn Future<Output = Result<T, E>>>>,
{
    let mut delay = initial_delay;
    for attempt in 0..max_retries {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) if attempt < max_retries - 1 => {
                tokio::time::sleep(delay).await;
                delay *= 2;  // 指数退避
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!()
}
```

#### 2. 断路器模式

```rust
pub struct CircuitBreaker {
    failure_threshold: u32,
    timeout: Duration,
    state: Arc<RwLock<CircuitState>>,
}

enum CircuitState {
    Closed { failures: u32 },
    Open { opened_at: Instant },
    HalfOpen,
}

impl CircuitBreaker {
    pub async fn call<F, T>(&self, f: F) -> Result<T, AppError>
    where
        F: Future<Output = Result<T, AppError>>,
    {
        let state = self.state.read().await;
        match *state {
            CircuitState::Open { opened_at } => {
                if opened_at.elapsed() > self.timeout {
                    drop(state);
                    *self.state.write().await = CircuitState::HalfOpen;
                } else {
                    return Err(AppError::CircuitBreakerOpen);
                }
            }
            _ => {}
        }
        drop(state);
        
        match f.await {
            Ok(result) => {
                *self.state.write().await = CircuitState::Closed { failures: 0 };
                Ok(result)
            }
            Err(e) => {
                let mut state = self.state.write().await;
                match *state {
                    CircuitState::Closed { failures } => {
                        if failures + 1 >= self.failure_threshold {
                            *state = CircuitState::Open { opened_at: Instant::now() };
                        } else {
                            *state = CircuitState::Closed { failures: failures + 1 };
                        }
                    }
                    CircuitState::HalfOpen => {
                        *state = CircuitState::Open { opened_at: Instant::now() };
                    }
                    _ => {}
                }
                Err(e)
            }
        }
    }
}
```

---

## 测试策略 (Testing Strategy)

### 测试金字塔

```
        ┌─────────────┐
        │   E2E 测试   │  5%
        │  (Tauri)    │
        └─────────────┘
      ┌─────────────────┐
      │   集成测试       │  15%
      │  (API + DB)    │
      └─────────────────┘
    ┌───────────────────────┐
    │      单元测试          │  80%
    │  (纯函数 + 组件)      │
    └───────────────────────┘
```

### 单元测试

**覆盖范围**:
- 加密算法切换逻辑
- DLP 脱敏规则匹配
- 审计日志摘要计算
- 配置解析和验证
- 错误处理逻辑

**示例**:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_dlp_sanitize_id_card() {
        let engine = DLPEngine::new();
        let text = "我的身份证号是 110101199001011234";
        let (sanitized, matches) = engine.sanitize(text);
        
        assert_eq!(sanitized, "我的身份证号是 ***************");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].rule_id, "id_card_cn");
    }
    
    #[test]
    fn test_crypto_provider_switch() {
        let default_provider = DefaultCryptoProvider::new();
        let gm_provider = GMCryptoProvider::new();
        
        let data = b"test data";
        let key = b"0123456789abcdef0123456789abcdef";
        
        let encrypted_default = default_provider.encrypt(key, data).unwrap();
        let encrypted_gm = gm_provider.encrypt(key, data).unwrap();
        
        // 验证两种算法产生不同的密文
        assert_ne!(encrypted_default, encrypted_gm);
        
        // 验证解密正确
        assert_eq!(default_provider.decrypt(key, &encrypted_default).unwrap(), data);
        assert_eq!(gm_provider.decrypt(key, &encrypted_gm).unwrap(), data);
    }
}
```

### 集成测试

**覆盖范围**:
- REST API 端到端流程
- 数据库事务一致性
- 插件上传和审核流程
- 审计日志同步流程

**示例**:

```rust
#[tokio::test]
async fn test_skill_upload_and_audit() {
    let app = test_app().await;
    
    // 1. 上传插件
    let response = app
        .post("/api/skills")
        .multipart(/* ... */)
        .await;
    assert_eq!(response.status(), 200);
    let skill_id = response.json::<SkillResponse>().await.id;
    
    // 2. 触发审核
    let audit_response = app
        .post(&format!("/api/skills/{}/audit", skill_id))
        .await;
    assert_eq!(audit_response.status(), 200);
    
    // 3. 验证审核结果
    let skill = app
        .get(&format!("/api/skills/{}", skill_id))
        .await
        .json::<Skill>()
        .await;
    assert_eq!(skill.status, "approved");
}
```

### 属性测试 (Property-Based Testing)

使用 `proptest` 库进行属性测试，验证系统的通用属性。

**配置**: 每个属性测试运行 100 次迭代

**标签格式**: `Feature: enterprise-ai-agent-platform, Property {number}: {property_text}`

我将在下一部分补充正确性属性。

---
## 正确性属性 (Correctness Properties)

属性是一个特征或行为，应该在系统的所有有效执行中保持为真——本质上是关于系统应该做什么的正式陈述。属性作为人类可读规范和机器可验证正确性保证之间的桥梁。

### 验收标准测试性分析 (Prework)

在定义正确性属性之前，我需要分析每个验收标准的可测试性：

#### 需求 1: 国密算法支持

1.1 系统应支持通过配置项在国密算法栈和默认算法栈之间切换
  思考: 这是一个配置解析和应用的测试。我们可以生成随机配置，验证系统使用了正确的算法栈。
  可测试性: yes - property

1.2 配置为国密模式时，安全内核应使用 SM2 进行数字签名验证
  思考: 这是验证算法选择的正确性。我们可以生成随机签名数据，验证使用了 SM2 算法。
  可测试性: yes - property

1.3 配置为国密模式时，安全内核应使用 SM4 进行本地数据加密
  思考: 这是加密算法的选择测试。可以通过加密/解密往返验证。
  可测试性: yes - property

1.4 配置为国密模式时，安全内核应使用 SM3 进行审计日志摘要计算
  思考: 这是哈希算法的选择测试。可以验证摘要长度和算法特征。
  可测试性: yes - property

1.5 系统应通过统一的 CryptoProvider 接口抽象加密算法实现
  思考: 这是架构设计要求，不是功能性测试。
  可测试性: no

#### 需求 2: Skills 安全审核流水线

2.1 开发者提交 Skill 时，管理后台应执行 SCA 供应链扫描
  思考: 这是审核流程的测试。可以提交包含已知漏洞的插件，验证是否被检测。
  可测试性: yes - example

2.2 开发者提交 Skill 时，管理后台应执行 AST 静态代码审计
  思考: 这是审核流程的测试。可以提交包含恶意代码的插件，验证是否被检测。
  可测试性: yes - example

2.3 Skill 通过所有安全审核时，管理后台应使用 SM2 私钥对插件进行数字签名
  思考: 这是签名生成的测试。对于任何通过审核的插件，都应该生成有效签名。
  可测试性: yes - property

2.4 Skill 未通过安全审核时，管理后台应拒绝上架并返回详细的审核报告
  思考: 这是错误处理的测试。对于任何未通过审核的插件，都应该返回报告。
  可测试性: yes - property

2.5 管理后台应记录所有审核过程和结果到审计日志
  思考: 这是审计日志完整性的测试。对于任何审核操作，都应该有对应的日志记录。
  可测试性: yes - property

#### 需求 3: 企业 Skills 商店管理

3.1 管理后台应支持插件的版本管理
  思考: 这是 CRUD 操作的测试。可以创建、更新、删除版本，验证操作成功。
  可测试性: yes - example

3.2 管理后台应支持为不同用户组配置插件访问权限
  思考: 这是权限配置的测试。可以配置权限，验证用户访问是否符合预期。
  可测试性: yes - example

3.3 管理员启动灰度发布时，管理后台应按照配置的用户组比例分阶段推送插件更新
  思考: 这是灰度发布逻辑的测试。可以验证推送比例是否正确。
  可测试性: yes - property

3.4 插件错误率超过阈值时，管理后台应支持管理员手动触发版本回滚
  思考: 这是回滚功能的测试。可以模拟错误率超标，验证回滚是否成功。
  可测试性: yes - example

3.5 管理后台应记录所有插件管理操作到审计日志
  思考: 这是审计日志完整性的测试。对于任何管理操作，都应该有对应的日志。
  可测试性: yes - property

#### 需求 4: ClawHub 插件市场集成

4.1 管理后台应支持手动刷新 ClawHub 插件清单
  思考: 这是 API 集成的测试。可以验证刷新后获取到最新清单。
  可测试性: yes - example

4.2 管理员选择特定插件时，管理后台应下载插件内容到企业内部存储
  思考: 这是下载功能的测试。可以验证文件是否正确下载和存储。
  可测试性: yes - example

4.3 ClawHub 插件下载完成时，管理后台应将其存入待审核池
  思考: 这是状态转换的测试。对于任何下载的插件，状态都应该是待审核。
  可测试性: yes - property

4.4 ClawHub 插件通过企业审核时，管理后台应使用企业 SM2 私钥进行二次签名
  思考: 这是签名流程的测试。对于任何通过审核的 ClawHub 插件，都应该有企业签名。
  可测试性: yes - property

4.5 桌面客户端应仅展示经过企业二次签名且状态为已审核的插件
  思考: 这是过滤逻辑的测试。客户端展示的插件集合应该是已审核插件的子集。
  可测试性: yes - property

#### 需求 5: 身份认证与会话管理

5.1 用户首次启动应用时，桌面客户端应要求用户设置主密码
  思考: 这是首次启动流程的测试。这是一个特定场景。
  可测试性: yes - example

5.2 用户设置主密码时，桌面客户端应将派生密钥存储在操作系统 Keychain 中
  思考: 这是密钥存储的测试。可以验证 Keychain 中是否存在派生密钥。
  可测试性: yes - example

5.3 用户启动应用时，桌面客户端应要求输入主密码进行解锁
  思考: 这是认证流程的测试。这是一个特定场景。
  可测试性: yes - example

5.4 用户无操作超过 30 分钟时，桌面客户端应自动锁定
  思考: 这是超时锁定的测试。可以模拟超时，验证是否自动锁定。
  可测试性: yes - example

5.5 用户执行敏感操作时，桌面客户端应要求二次验证主密码
  思考: 这是二次验证的测试。对于任何敏感操作，都应该触发二次验证。
  可测试性: yes - property

5.6 桌面客户端应使用 JWT 作为访问令牌，有效期为 1 小时
  思考: 这是令牌格式和有效期的测试。可以验证令牌是否为 JWT 格式且有效期正确。
  可测试性: yes - property

5.7 桌面客户端应使用 Refresh Token 进行令牌续期，有效期为 7 天
  思考: 这是令牌续期的测试。可以验证续期逻辑是否正确。
  可测试性: yes - property

#### 需求 6: DLP 数据脱敏

6.1 管理后台应支持配置和分发 DLP 脱敏词库
  思考: 这是配置管理的测试。可以验证词库是否正确分发。
  可测试性: yes - example

6.2 用户输入包含敏感信息时，安全内核应根据 DLP 词库实时过滤敏感内容
  思考: 这是脱敏逻辑的测试。对于任何包含敏感信息的输入，都应该被过滤。
  可测试性: yes - property

6.3 模型推理请求包含敏感信息时，安全内核应在发送前进行脱敏处理
  思考: 这是脱敏逻辑的测试。对于任何推理请求，敏感信息都应该被脱敏。
  可测试性: yes - property

6.4 DLP 词库更新时，桌面客户端应自动同步最新词库到本地
  思考: 这是同步逻辑的测试。可以验证更新后客户端是否获取到最新词库。
  可测试性: yes - example

6.5 系统处于离线状态时，安全内核应使用本地缓存的 DLP 词库进行脱敏
  思考: 这是离线模式的测试。可以验证离线时是否使用缓存词库。
  可测试性: yes - example

#### 需求 7: 敏感操作物理拦截

7.1 AI Agent 尝试执行敏感操作时，桌面客户端应暂停执行并展示授权按钮
  思考: 这是拦截逻辑的测试。对于任何敏感操作，都应该触发拦截。
  可测试性: yes - property

7.2 桌面客户端应在对话卡片中清晰展示操作类型、影响范围和风险等级
  思考: 这是 UI 展示的测试。可以验证展示内容是否包含必要信息。
  可测试性: yes - property

7.3 用户确认操作时，桌面客户端应继续执行并记录审批结果到审计日志
  思考: 这是审批流程的测试。对于任何确认的操作，都应该有审计日志。
  可测试性: yes - property

7.4 用户拒绝操作时，桌面客户端应取消执行并记录拒绝原因到审计日志
  思考: 这是审批流程的测试。对于任何拒绝的操作，都应该有审计日志。
  可测试性: yes - property

7.5 管理后台应支持配置哪些操作类型需要人工审批
  思考: 这是配置管理的测试。可以验证配置是否生效。
  可测试性: yes - example

#### 需求 8: WASM 插件隔离执行

8.1 用户加载 WASM 插件时，安全内核应验证插件的 SM2 数字签名
  思考: 这是签名验证的测试。对于任何插件，都应该验证签名。
  可测试性: yes - property

8.2 签名验证通过时，安全内核应将插件加载到 Wasmtime 运行时沙箱中
  思考: 这是加载逻辑的测试。对于任何验证通过的插件，都应该成功加载。
  可测试性: yes - property

8.3 签名验证失败时，安全内核应拒绝加载并记录安全事件到审计日志
  思考: 这是错误处理的测试。对于任何验证失败的插件，都应该拒绝并记录。
  可测试性: yes - property

8.4 WASM 插件运行时，安全内核应限制其访问的系统资源
  思考: 这是资源限制的测试。可以验证插件是否无法访问受限资源。
  可测试性: yes - property

8.5 WASM 插件尝试访问受限资源时，安全内核应拦截请求并触发审批流程
  思考: 这是拦截逻辑的测试。对于任何受限资源访问，都应该触发审批。
  可测试性: yes - property

#### 需求 9: MCP 协议网桥

9.1 系统应支持通过 MCP 协议与外挂沙箱中的脚本插件通信
  思考: 这是协议支持的测试。可以验证 MCP 通信是否正常。
  可测试性: yes - example

9.2 用户调用脚本插件时，安全内核应通过 MCP Bridge 路由请求到对应的沙箱环境
  思考: 这是路由逻辑的测试。对于任何脚本插件调用，都应该正确路由。
  可测试性: yes - property

9.3 MCP Bridge 应记录所有脚本插件的调用参数、返回值和执行时间到审计日志
  思考: 这是审计日志完整性的测试。对于任何 MCP 调用，都应该有日志记录。
  可测试性: yes - property

9.4 脚本插件尝试执行敏感操作时，MCP Bridge 应拦截请求并触发审批流程
  思考: 这是拦截逻辑的测试。对于任何敏感操作，都应该触发审批。
  可测试性: yes - property

9.5 MCP Bridge 应支持同步和异步调用模式
  思考: 这是调用模式的测试。可以验证两种模式是否都正常工作。
  可测试性: yes - example

#### 需求 10: 本地加密存储

10.1 桌面客户端应使用主密码派生的密钥对 libSQL 数据库进行全库加密
  思考: 这是加密存储的测试。可以验证数据库是否被加密。
  可测试性: yes - example

10.2 用户输入主密码解锁时，桌面客户端应使用派生密钥解密数据库
  思考: 这是解密逻辑的测试。这是加密/解密的往返属性。
  可测试性: yes - property

10.3 桌面客户端应使用 SM4 或 AES-256-GCM 加密本地配置文件
  思考: 这是加密算法选择的测试。可以验证使用了正确的算法。
  可测试性: yes - property

10.4 桌面客户端应使用 SM4 或 AES-256-GCM 加密临时审计日志
  思考: 这是加密算法选择的测试。可以验证使用了正确的算法。
  可测试性: yes - property

10.5 加密或解密失败时，桌面客户端应记录错误并拒绝访问数据
  思考: 这是错误处理的测试。对于任何加密/解密失败，都应该拒绝访问。
  可测试性: yes - property

#### 需求 11: 离线审计与同步

11.1 系统处于离线状态时，桌面客户端应将审计日志加密存储到本地数据库
  思考: 这是离线存储的测试。可以验证离线时日志是否被加密存储。
  可测试性: yes - example

11.2 桌面客户端应使用 SM3 或 SHA256 对每条审计日志计算摘要
  思考: 这是摘要计算的测试。对于任何审计日志，都应该有摘要。
  可测试性: yes - property

11.3 系统恢复网络连接时，桌面客户端应自动检测并启动日志同步流程
  思考: 这是自动同步的测试。可以验证连线后是否自动同步。
  可测试性: yes - example

11.4 日志同步时，桌面客户端应使用断点续传机制，仅上传未同步的日志
  思考: 这是断点续传的测试。可以验证是否只上传未同步的日志。
  可测试性: yes - property

11.5 日志同步完成时，管理后台应验证日志摘要并存储到审计仓库
  思考: 这是摘要验证的测试。对于任何同步的日志，都应该验证摘要。
  可测试性: yes - property

11.6 日志摘要验证失败时，管理后台应记录安全事件并通知管理员
  思考: 这是错误处理的测试。对于任何验证失败，都应该记录并通知。
  可测试性: yes - property

#### 需求 12: 安全推理网关

12.1 系统应支持配置统一的推理网关地址
  思考: 这是配置管理的测试。可以验证配置是否生效。
  可测试性: yes - example

12.2 用户发起推理请求时，桌面客户端应将请求路由到网关
  思考: 这是路由逻辑的测试。对于任何推理请求，都应该路由到网关。
  可测试性: yes - property

12.3 系统应确保所有通过网关的请求都已执行 DLP 脱敏处理
  思考: 这是脱敏保证的测试。对于任何网关请求，都应该已脱敏。
  可测试性: yes - property

12.4 系统应支持在网关层实现多租户配额管理和统一 API Key 注入
  思考: 这是网关功能的测试。可以验证配额和 API Key 是否正确处理。
  可测试性: yes - example

12.5 系统处于离线状态时，桌面客户端应通过网关路由至本地部署的 LLM
  思考: 这是离线路由的测试。可以验证离线时是否路由到本地 LLM。
  可测试性: yes - example

#### 需求 13: 动态水印

13.1 桌面客户端应在对话界面上叠加包含用户名和用户 ID 的半透明水印
  思考: 这是 UI 渲染的测试。可以验证水印是否包含必要信息。
  可测试性: yes - property

13.2 桌面客户端应使用 Canvas API 实现水印渲染，透明度为 10-20%
  思考: 这是实现细节的测试。可以验证透明度是否在范围内。
  可测试性: yes - property

13.3 桌面客户端应随机平铺或对角线重复水印
  思考: 这是渲染策略的测试。可以验证水印是否按策略渲染。
  可测试性: yes - property

13.4 桌面客户端应确保水印不影响用户正常阅读和操作
  思考: 这是用户体验的测试，难以自动化验证。
  可测试性: no

#### 需求 14: 思考链展示

14.1 AI 模型生成思考链时，桌面客户端应在对话界面中展示思考过程
  思考: 这是 UI 展示的测试。可以验证思考链是否被展示。
  可测试性: yes - property

14.2 桌面客户端应使用可折叠的 UI 组件展示思考链
  思考: 这是 UI 组件的测试。可以验证组件是否可折叠。
  可测试性: yes - example

14.3 桌面客户端应支持用户展开或折叠思考链内容
  思考: 这是交互功能的测试。可以验证展开/折叠是否正常工作。
  可测试性: yes - example

14.4 桌面客户端应使用不同的视觉样式区分思考链和最终回复
  思考: 这是 UI 样式的测试。可以验证样式是否不同。
  可测试性: yes - property

14.5 桌面客户端应记录用户是否查看了思考链到审计日志
  思考: 这是审计日志的测试。对于任何思考链交互，都应该有日志。
  可测试性: yes - property

#### 需求 15: 离线工作模式

15.1 系统处于离线状态时，桌面客户端应支持使用主密码进行身份认证
  思考: 这是离线认证的测试。可以验证离线时认证是否正常。
  可测试性: yes - example

15.2 系统处于离线状态时，桌面客户端应支持加载已下载的插件并验证签名
  思考: 这是离线插件加载的测试。可以验证离线时是否能加载插件。
  可测试性: yes - example

15.3 系统处于离线状态时，桌面客户端应支持使用本地缓存的 DLP 词库进行脱敏
  思考: 这是离线脱敏的测试。可以验证离线时脱敏是否正常。
  可测试性: yes - example

15.4 系统处于离线状态时，桌面客户端应支持使用本地部署的 LLM 进行推理
  思考: 这是离线推理的测试。可以验证离线时推理是否正常。
  可测试性: yes - example

15.5 系统处于离线状态时，桌面客户端应支持执行本地工具
  思考: 这是离线工具执行的测试。可以验证离线时本地工具是否可用。
  可测试性: yes - example

15.6 系统处于离线状态时，桌面客户端应拒绝执行需要网络连接的工具
  思考: 这是离线限制的测试。对于任何需要网络的工具，都应该被拒绝。
  可测试性: yes - property

15.7 系统处于离线状态时，桌面客户端应将审计日志加密存储到本地
  思考: 这是离线审计的测试。可以验证离线时日志是否被存储。
  可测试性: yes - example

#### 需求 16: 策略引擎

16.1 管理后台应支持配置 DLP 脱敏词库
  思考: 这是配置管理的测试。可以验证配置是否成功。
  可测试性: yes - example

16.2 管理后台应支持配置敏感操作拦截规则
  思考: 这是配置管理的测试。可以验证配置是否成功。
  可测试性: yes - example

16.3 策略更新时，管理后台应将最新策略推送到所有在线的桌面客户端
  思考: 这是策略分发的测试。可以验证在线客户端是否收到更新。
  可测试性: yes - example

16.4 桌面客户端连线时，应检查本地策略版本并自动同步最新策略
  思考: 这是策略同步的测试。可以验证连线后是否同步最新策略。
  可测试性: yes - example

16.5 管理后台应记录所有策略配置和分发操作到审计日志
  思考: 这是审计日志的测试。对于任何策略操作，都应该有日志。
  可测试性: yes - property

#### 需求 17: 行为审计中心

17.1 管理后台应接收并存储来自所有桌面客户端的审计日志
  思考: 这是日志接收的测试。可以验证日志是否被正确存储。
  可测试性: yes - example

17.2 管理后台应验证每条审计日志的摘要，确保日志未被篡改
  思考: 这是摘要验证的测试。对于任何接收的日志，都应该验证摘要。
  可测试性: yes - property

17.3 管理后台应支持按用户、时间范围、操作类型等维度查询审计日志
  思考: 这是查询功能的测试。可以验证查询是否返回正确结果。
  可测试性: yes - example

17.4 管理后台应支持配置违规行为检测规则
  思考: 这是配置管理的测试。可以验证配置是否成功。
  可测试性: yes - example

17.5 检测到违规行为时，管理后台应生成告警并通知管理员
  思考: 这是告警逻辑的测试。对于任何违规行为，都应该生成告警。
  可测试性: yes - property

17.6 管理后台应将审计日志存储到时序数据库
  思考: 这是存储实现的测试。可以验证日志是否存储到正确的数据库。
  可测试性: yes - example

#### 需求 18: 配置解析器与格式化器

18.1 系统加载配置文件时，应解析配置文件并验证其格式和内容
  思考: 这是配置解析的测试。可以验证解析是否正确。
  可测试性: yes - property

18.2 配置文件格式错误时，系统应返回详细的错误信息
  思考: 这是错误处理的测试。对于任何格式错误，都应该返回错误信息。
  可测试性: yes - property

18.3 系统应支持将配置对象格式化为标准的配置文件格式
  思考: 这是格式化功能的测试。可以验证格式化是否正确。
  可测试性: yes - property

18.4 对于所有有效的配置对象，解析、格式化、再解析后应得到等价的配置对象
  思考: 这是往返属性的测试。这是一个经典的往返属性。
  可测试性: yes - property

18.5 系统应记录所有配置解析错误到日志
  思考: 这是日志记录的测试。对于任何解析错误，都应该有日志。
  可测试性: yes - property

#### 需求 19: 插件版本管理

19.1 管理后台应支持为每个插件存储多个版本
  思考: 这是版本存储的测试。可以验证多版本是否正确存储。
  可测试性: yes - example

19.2 管理后台应为每个插件版本记录版本号、发布时间、变更日志和签名
  思考: 这是元数据记录的测试。对于任何版本，都应该有完整元数据。
  可测试性: yes - property

19.3 管理员发布新版本时，管理后台应验证版本号的唯一性
  思考: 这是唯一性约束的测试。对于任何新版本，版本号都应该唯一。
  可测试性: yes - property

19.4 管理后台应支持将插件回滚到历史版本
  思考: 这是回滚功能的测试。可以验证回滚是否成功。
  可测试性: yes - example

19.5 桌面客户端应支持查看插件的版本历史和变更日志
  思考: 这是查询功能的测试。可以验证是否能查看历史。
  可测试性: yes - example

19.6 插件有新版本可用时，桌面客户端应提示用户更新
  思考: 这是通知功能的测试。对于任何新版本，都应该提示用户。
  可测试性: yes - property

#### 需求 20: 灰度发布策略

20.1 管理后台应支持配置灰度发布策略
  思考: 这是配置管理的测试。可以验证配置是否成功。
  可测试性: yes - example

20.2 管理后台应支持为每个发布阶段配置目标用户组和推送比例
  思考: 这是配置管理的测试。可以验证配置是否成功。
  可测试性: yes - example

20.3 管理员启动灰度发布时，管理后台应按照配置的阶段顺序逐步推送更新
  思考: 这是发布流程的测试。可以验证推送顺序是否正确。
  可测试性: yes - property

20.4 管理后台应监控每个发布阶段的插件错误率和用户反馈
  思考: 这是监控功能的测试。可以验证监控是否正常工作。
  可测试性: yes - example

20.5 错误率超过配置的阈值时，管理后台应自动暂停发布并通知管理员
  思考: 这是自动暂停的测试。对于任何超阈值情况，都应该暂停。
  可测试性: yes - property

20.6 管理后台应支持管理员手动推进到下一阶段或回滚到上一版本
  思考: 这是手动控制的测试。可以验证手动操作是否成功。
  可测试性: yes - example


### 属性反思 (Property Reflection)

在将验收标准转换为正确性属性之前，我需要识别并消除冗余属性：

**冗余分析**:

1. **加密算法选择** (需求 1.2, 1.3, 1.4, 10.3, 10.4)
   - 这些都是验证配置模式下使用正确算法的测试
   - 可以合并为一个综合属性：配置模式决定算法选择

2. **审计日志完整性** (需求 2.5, 3.5, 7.3, 7.4, 9.3, 16.5, 18.5)
   - 这些都是验证操作被记录到审计日志的测试
   - 可以合并为一个综合属性：所有关键操作都应该有审计日志

3. **签名验证流程** (需求 2.3, 4.4, 8.1)
   - 这些都是验证签名生成和验证的测试
   - 可以合并为一个综合属性：通过审核的插件都应该有有效签名

4. **敏感操作拦截** (需求 7.1, 8.5, 9.4)
   - 这些都是验证敏感操作触发审批的测试
   - 可以合并为一个综合属性：所有敏感操作都应该触发审批流程

5. **DLP 脱敏保证** (需求 6.2, 6.3, 12.3)
   - 这些都是验证敏感信息被脱敏的测试
   - 可以合并为一个综合属性：所有输出都应该经过 DLP 脱敏

6. **摘要验证** (需求 11.2, 11.5, 17.2)
   - 这些都是验证审计日志摘要的测试
   - 可以合并为一个综合属性：所有审计日志都应该有有效摘要

**保留的独特属性**:

- 配置往返属性 (需求 18.4) - 经典往返属性
- 加密/解密往返属性 (需求 10.2) - 经典往返属性
- 断点续传逻辑 (需求 11.4) - 独特的同步逻辑
- 灰度发布顺序 (需求 20.3) - 独特的发布流程
- 版本唯一性 (需求 19.3) - 独特的约束验证
- JWT 令牌有效期 (需求 5.6, 5.7) - 独特的令牌管理



---

## 正确性属性 (Correctness Properties)

属性是一个特征或行为，应该在系统的所有有效执行中保持为真——本质上是关于系统应该做什么的正式陈述。属性作为人类可读规范和机器可验证正确性保证之间的桥梁。

基于验收标准分析和属性反思，以下是系统的核心正确性属性：

### 属性 1: 加密算法配置一致性

*对于任何* 加密操作（签名验证、数据加密、摘要计算），当系统配置为国密模式时，应使用国密算法栈（SM2/SM3/SM4）；当配置为默认模式时，应使用默认算法栈（Ed25519/SHA256/AES-256-GCM）

**验证需求**: 1.1, 1.2, 1.3, 1.4

### 属性 2: DLP 脱敏完整性

*对于任何* 发送到外部系统的数据（模型推理请求、网络请求、日志上报），如果包含匹配 DLP 规则的敏感信息，则该信息应被脱敏替换

**验证需求**: 6.2, 6.3, 12.3

### 属性 3: 敏感操作强制审批

*对于任何* 被标记为敏感的操作（文件删除、外网访问、受限资源访问），系统应暂停执行并要求用户审批，只有在用户明确确认后才继续执行

**验证需求**: 7.1, 8.5, 9.4

### 属性 4: 审计日志完整性

*对于任何* 关键操作（插件审核、策略配置、用户审批、MCP 调用），系统应生成审计日志条目并持久化存储

**验证需求**: 2.5, 3.5, 7.3, 7.4, 9.3, 16.5

### 属性 5: 插件签名验证

*对于任何* 加载到系统中的插件，必须通过数字签名验证；签名验证失败的插件应被拒绝加载并记录安全事件

**验证需求**: 2.3, 4.4, 8.1, 8.3

### 属性 6: 审计日志摘要防篡改

*对于任何* 审计日志条目，系统应计算其摘要（SM3 或 SHA256）；在日志同步时，接收方应验证摘要一致性，摘要不匹配的日志应被标记为可疑

**验证需求**: 11.2, 11.5, 17.2

### 属性 7: 配置解析往返属性

*对于任何* 有效的配置对象，执行"序列化 → 解析 → 序列化"操作后，应得到与原始序列化结果等价的配置

**验证需求**: 18.4


### 属性 8: 加密解密往返属性

*对于任何* 数据和有效密钥，执行"加密 → 解密"操作后，应得到与原始数据相同的结果

**验证需求**: 10.2

### 属性 9: 断点续传增量同步

*对于任何* 审计日志同步操作，系统应只上传标记为"未同步"的日志条目；同步成功后，这些条目应被标记为"已同步"

**验证需求**: 11.4

### 属性 10: 插件版本唯一性

*对于任何* 插件，其所有版本的版本号应唯一；尝试创建重复版本号的版本应被拒绝

**验证需求**: 19.3

### 属性 11: 灰度发布阶段顺序

*对于任何* 灰度发布流程，系统应按照配置的阶段顺序（Alpha → Beta → GA）逐步推送更新；不应跳过中间阶段

**验证需求**: 20.3

### 属性 12: 错误率阈值自动暂停

*对于任何* 灰度发布阶段，如果插件错误率超过配置的阈值，系统应自动暂停发布并通知管理员

**验证需求**: 20.5

### 属性 13: 离线模式网络工具拒绝

*对于任何* 需要网络连接的工具调用，当系统处于离线状态时，应拒绝执行并返回明确的错误信息

**验证需求**: 15.6

### 属性 14: 加密失败访问拒绝

*对于任何* 加密或解密操作失败的情况，系统应拒绝访问相关数据并记录错误日志

**验证需求**: 10.5

### 属性 15: 配置解析错误详细报告

*对于任何* 格式错误的配置文件，系统应返回包含错误位置和原因的详细错误信息

**验证需求**: 18.2


---

## 技术决策 (Technical Decisions)

### 1. 为什么选择 Tauri 而不是 Electron？

**决策**: 使用 Tauri 2.x 作为桌面应用框架

**理由**:
- **安全性**: Tauri 使用系统原生 WebView，攻击面更小
- **性能**: 内存占用约为 Electron 的 1/10，启动速度更快
- **体积**: 安装包体积约为 Electron 的 1/5
- **Rust 生态**: 与后端技术栈统一，便于代码复用
- **国密集成**: Rust FFI 调用 C 库更直接，性能更好

**权衡**:
- 社区生态相对较小
- 部分 Electron 插件无法直接迁移
- 需要团队学习 Rust

### 2. 为什么使用 libSQL 而不是 SQLite？

**决策**: 桌面客户端使用 libSQL 作为嵌入式数据库

**理由**:
- **加密支持**: 原生支持全库加密
- **兼容性**: 完全兼容 SQLite API
- **现代化**: 支持 WASM、异步 I/O
- **维护性**: 活跃的开源社区

**权衡**:
- 相对较新，生产案例较少
- 部分高级特性仍在开发中

### 3. 为什么选择 ClickHouse/TimescaleDB？

**决策**: 审计日志使用时序数据库存储

**理由**:
- **写入性能**: 支持高吞吐批量写入
- **查询性能**: 针对时间范围查询优化
- **压缩率**: 列式存储，压缩率高
- **扩展性**: 支持水平扩展

**权衡**:
- 运维复杂度较高
- 不适合频繁更新的场景


### 4. 为什么使用 Wasmtime 而不是其他 WASM 运行时？

**决策**: 使用 Wasmtime 作为 WASM 插件运行时

**理由**:
- **安全性**: 字节码联盟（Bytecode Alliance）维护，安全审计严格
- **性能**: JIT 编译，性能接近原生代码
- **WASI 支持**: 完整支持 WASI 标准
- **资源限制**: 支持内存、CPU、I/O 限制
- **Rust 集成**: 官方 Rust API，集成简单

**权衡**:
- 二进制体积较大
- 部分实验性特性不稳定

### 5. 为什么引入 MCP 协议？

**决策**: 支持 MCP 协议与脚本插件通信

**理由**:
- **兼容性**: ClawHub 社区存在大量 Python/Node.js 脚本
- **标准化**: MCP 是 Anthropic 推出的标准协议
- **隔离性**: 脚本在独立进程中运行，崩溃不影响主程序
- **灵活性**: 支持多种运行时（Python、Node.js、Ruby）

**权衡**:
- 进程间通信开销
- 需要管理子进程生命周期

### 6. 为什么采用主密码而不是 SSO（MVP 阶段）？

**决策**: MVP 阶段使用主密码认证，P2 阶段引入 SSO

**理由**:
- **实现简单**: 主密码方案实现周期短
- **离线支持**: 无需网络连接即可认证
- **用户体验**: 单一密码，易于记忆
- **安全性**: 结合 OS Keychain，安全性可接受

**权衡**:
- 无法集成企业现有 IAM 系统
- 密码管理负担在用户侧
- 不支持多因素认证


---

## 安全设计补充 (Security Design Details)

### 国密算法集成方案

#### FFI 绑定设计

```rust
// src/crypto/gmssl_ffi.rs
use std::ffi::{c_void, CString};
use std::os::raw::{c_char, c_int, c_uchar};

#[link(name = "gmssl")]
extern "C" {
    // SM2 签名验证
    fn SM2_verify(
        dgst: *const c_uchar,
        dgstlen: c_int,
        sig: *const c_uchar,
        siglen: c_int,
        pub_key: *const c_void,
    ) -> c_int;
    
    // SM4 加密
    fn SM4_encrypt(
        in_data: *const c_uchar,
        in_len: c_int,
        key: *const c_uchar,
        out_data: *mut c_uchar,
    ) -> c_int;
    
    // SM3 哈希
    fn SM3_hash(
        data: *const c_uchar,
        len: c_int,
        hash: *mut c_uchar,
    ) -> c_int;
}
```

#### 安全内存管理

```rust
// 敏感数据自动清零
pub struct SecureBytes {
    data: Vec<u8>,
}

impl Drop for SecureBytes {
    fn drop(&mut self) {
        // 使用 volatile 写入防止编译器优化
        for byte in &mut self.data {
            unsafe {
                std::ptr::write_volatile(byte, 0);
            }
        }
    }
}
```


### 签名验证流程

```
┌─────────────────────────────────────────────────────────────┐
│                    插件签名验证流程                           │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  1. 读取插件二进制文件                                        │
│     ↓                                                         │
│  2. 计算文件哈希 (SM3/SHA256)                                │
│     ↓                                                         │
│  3. 读取签名文件                                              │
│     ↓                                                         │
│  4. 使用公钥验证签名                                          │
│     ├─ 验证通过 → 加载插件                                    │
│     └─ 验证失败 → 拒绝加载 + 记录安全事件                     │
│                                                               │
└─────────────────────────────────────────────────────────────┘
```

### DLP 规则引擎优化

#### 规则编译

```rust
pub struct CompiledDLPRule {
    id: String,
    pattern: Regex,
    replacement: String,
    severity: Severity,
    // 预编译的 DFA 状态机
    dfa: Option<DFA>,
}

impl DLPEngine {
    /// 编译规则以提升性能
    pub fn compile_rules(&mut self) -> Result<()> {
        for rule in &mut self.rules {
            // 将正则表达式编译为 DFA
            rule.dfa = Some(DFA::from_regex(&rule.pattern)?);
        }
        Ok(())
    }
}
```

---

## 部署架构补充 (Deployment Architecture Details)

### 高可用部署

```
┌─────────────────────────────────────────────────────────────┐
│                    管理后台高可用架构                         │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌─────────────┐         ┌─────────────┐                    │
│  │  Nginx LB   │────────▶│  Nginx LB   │  (主备)            │
│  └──────┬──────┘         └─────────────┘                    │
│         │                                                     │
│         ├──────────┬──────────┬──────────┐                  │
│         ▼          ▼          ▼          ▼                  │
│  ┌──────────┐┌──────────┐┌──────────┐┌──────────┐         │
│  │ Backend  ││ Backend  ││ Backend  ││ Backend  │         │
│  │ Node 1   ││ Node 2   ││ Node 3   ││ Node 4   │         │
│  └────┬─────┘└────┬─────┘└────┬─────┘└────┬─────┘         │
│       │           │           │           │                 │
│       └───────────┴───────────┴───────────┘                 │
│                       │                                      │
│                       ▼                                      │
│  ┌─────────────────────────────────────────────┐           │
│  │  PostgreSQL 主从集群                         │           │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐    │           │
│  │  │ Primary │─▶│ Standby │─▶│ Standby │    │           │
│  │  └─────────┘  └─────────┘  └─────────┘    │           │
│  └─────────────────────────────────────────────┘           │
│                                                               │
└─────────────────────────────────────────────────────────────┘
```


### 容器化部署

```yaml
# docker-compose.yml
version: '3.8'

services:
  admin-backend:
    image: enterprise-ai-agent/admin-backend:latest
    ports:
      - "8080:8080"
    environment:
      - DATABASE_URL=postgresql://user:pass@postgres:5432/admin
      - CRYPTO_STANDARD=ChinaCrypto
      - REDIS_URL=redis://redis:6379
    volumes:
      - ./config:/app/config
      - ./skills:/app/skills
    depends_on:
      - postgres
      - redis
    restart: unless-stopped

  postgres:
    image: postgres:15
    environment:
      - POSTGRES_DB=admin
      - POSTGRES_USER=user
      - POSTGRES_PASSWORD=pass
    volumes:
      - postgres-data:/var/lib/postgresql/data
    restart: unless-stopped

  clickhouse:
    image: clickhouse/clickhouse-server:latest
    ports:
      - "8123:8123"
      - "9000:9000"
    volumes:
      - clickhouse-data:/var/lib/clickhouse
    restart: unless-stopped

  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    volumes:
      - redis-data:/data
    restart: unless-stopped

volumes:
  postgres-data:
  clickhouse-data:
  redis-data:
```

---

## 性能优化 (Performance Optimization)

### 1. DLP 脱敏性能优化

**策略**:
- 使用 Aho-Corasick 算法进行多模式匹配
- 规则预编译为 DFA 状态机
- LRU 缓存脱敏结果

**预期性能**:
- 单次脱敏延迟: < 1ms (1KB 文本)
- 吞吐量: > 10,000 次/秒

### 2. 审计日志批量写入

**策略**:
- 本地缓冲区聚合日志
- 批量上报 (100 条/批次)
- 异步写入，不阻塞主流程

**预期性能**:
- 日志写入延迟: < 10ms (P99)
- 吞吐量: > 50,000 条/秒

### 3. WASM 插件加载优化

**策略**:
- 插件预编译缓存
- 延迟加载非关键插件
- 共享 WASM 模块实例

**预期性能**:
- 首次加载: < 100ms
- 缓存加载: < 10ms


---

## 监控与可观测性 (Monitoring and Observability)

### 关键指标

#### 管理后台

| 指标 | 说明 | 告警阈值 |
|------|------|---------|
| API 响应时间 | P99 延迟 | > 500ms |
| 插件审核队列长度 | 待审核插件数量 | > 100 |
| 数据库连接池使用率 | 连接池占用比例 | > 80% |
| 审计日志写入失败率 | 日志写入失败比例 | > 1% |

#### 桌面客户端

| 指标 | 说明 | 告警阈值 |
|------|------|---------|
| 内存占用 | 进程内存使用 | > 500MB |
| DLP 脱敏延迟 | P99 延迟 | > 10ms |
| 插件执行超时率 | 超时比例 | > 5% |
| 离线日志积压 | 未同步日志数量 | > 1000 |

### 日志规范

```rust
// 结构化日志示例
log::info!(
    target: "audit",
    user_id = %user_id,
    operation = "plugin_load",
    plugin_id = %plugin_id,
    result = "success";
    "Plugin loaded successfully"
);

log::error!(
    target: "security",
    user_id = %user_id,
    operation = "signature_verify",
    plugin_id = %plugin_id,
    error = %err;
    "Signature verification failed"
);
```

### 分布式追踪

使用 OpenTelemetry 进行分布式追踪：

```rust
use opentelemetry::trace::{Tracer, SpanKind};

let tracer = global::tracer("admin-backend");
let span = tracer
    .span_builder("plugin_audit")
    .with_kind(SpanKind::Internal)
    .start(&tracer);

// 执行审核逻辑
let result = audit_plugin(&plugin).await;

span.set_attribute(KeyValue::new("plugin.id", plugin.id.clone()));
span.set_attribute(KeyValue::new("audit.result", result.status.clone()));
span.end();
```


---

## 测试策略补充 (Testing Strategy Details)

### 属性测试配置

使用 `proptest` 进行属性测试：

```rust
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]
    
    /// Feature: enterprise-ai-agent-platform, Property 7: 配置解析往返属性
    #[test]
    fn test_config_roundtrip(config in arb_config()) {
        let serialized = serialize_config(&config)?;
        let parsed = parse_config(&serialized)?;
        let reserialized = serialize_config(&parsed)?;
        
        prop_assert_eq!(serialized, reserialized);
    }
    
    /// Feature: enterprise-ai-agent-platform, Property 8: 加密解密往返属性
    #[test]
    fn test_crypto_roundtrip(
        data in prop::collection::vec(any::<u8>(), 0..1024),
        key in prop::array::uniform32(any::<u8>())
    ) {
        let crypto = get_crypto_provider();
        let encrypted = crypto.encrypt(&key, &data)?;
        let decrypted = crypto.decrypt(&key, &encrypted)?;
        
        prop_assert_eq!(data, decrypted);
    }
}
```

### 集成测试环境

```rust
// tests/integration/mod.rs
use testcontainers::{clients, images};

#[tokio::test]
async fn test_full_workflow() {
    // 启动测试容器
    let docker = clients::Cli::default();
    let postgres = docker.run(images::postgres::Postgres::default());
    let redis = docker.run(images::redis::Redis::default());
    
    // 初始化测试应用
    let app = TestApp::new()
        .with_postgres(&postgres)
        .with_redis(&redis)
        .build()
        .await;
    
    // 执行测试
    let skill = app.upload_skill("test-skill.wasm").await?;
    assert_eq!(skill.status, "pending");
    
    app.trigger_audit(&skill.id).await?;
    let audited_skill = app.get_skill(&skill.id).await?;
    assert_eq!(audited_skill.status, "approved");
}
```


### E2E 测试

使用 Tauri 的测试框架进行端到端测试：

```rust
// tests/e2e/auth.rs
use tauri_driver::Driver;

#[tokio::test]
async fn test_master_password_auth() {
    let driver = Driver::new().await?;
    
    // 首次启动，设置主密码
    driver.find_element("#master-password-input").await?
        .send_keys("test-password-123").await?;
    driver.find_element("#confirm-password-input").await?
        .send_keys("test-password-123").await?;
    driver.find_element("#set-password-btn").await?
        .click().await?;
    
    // 验证进入主界面
    let chat_input = driver.find_element("#chat-input").await?;
    assert!(chat_input.is_displayed().await?);
    
    // 锁定会话
    driver.find_element("#lock-session-btn").await?
        .click().await?;
    
    // 验证需要重新解锁
    let unlock_input = driver.find_element("#unlock-password-input").await?;
    assert!(unlock_input.is_displayed().await?);
}
```

---

## 开发路线图 (Development Roadmap)

### MVP 阶段 (P0 - 3 个月)

**目标**: 核心安全功能和基础架构

- ✅ 国密算法集成 (SM2/SM3/SM4)
- ✅ 主密码认证
- ✅ DLP 脱敏引擎
- ✅ WASM 沙箱隔离
- ✅ 敏感操作物理拦截
- ✅ 审计日志本地存储
- ✅ 插件签名验证
- ✅ 管理后台基础功能

### P1 阶段 (4-6 个月)

**目标**: 完善审计和插件生态

- ⬜ 离线审计同步
- ⬜ 断点续传机制
- ⬜ MCP 协议网桥
- ⬜ ClawHub 插件市场集成
- ⬜ 灰度发布系统
- ⬜ 策略引擎自动同步
- ⬜ 行为审计中心
- ⬜ 违规检测规则

### P2 阶段 (7-9 个月)

**目标**: 企业级功能和高级特性

- ⬜ SSO 集成 (SAML/OAuth2)
- ⬜ UKey 硬件认证
- ⬜ RBAC 权限系统
- ⬜ 多租户隔离
- ⬜ 高可用部署
- ⬜ 性能优化
- ⬜ 监控告警系统
- ⬜ 完整文档和培训材料


---

## 附录 (Appendix)

### A. 术语表

| 术语 | 英文 | 说明 |
|------|------|------|
| 国密算法 | GM Crypto | 中国商用密码算法，包括 SM2/SM3/SM4 |
| DLP | Data Loss Prevention | 数据泄露防护 |
| MCP | Model Context Protocol | 模型上下文协议 |
| WASM | WebAssembly | Web 汇编，跨平台字节码格式 |
| WASI | WebAssembly System Interface | WASM 系统接口标准 |
| CoT | Chain of Thought | 思考链 |
| SCA | Software Composition Analysis | 软件成分分析 |
| AST | Abstract Syntax Tree | 抽象语法树 |

### B. 参考资料

#### 国密算法

- [GM/T 0003-2012 SM2 椭圆曲线公钥密码算法](http://www.gmbz.org.cn/main/viewfile/20180108023812835219.html)
- [GM/T 0004-2012 SM3 密码杂凑算法](http://www.gmbz.org.cn/main/viewfile/20180108023812835219.html)
- [GM/T 0002-2012 SM4 分组密码算法](http://www.gmbz.org.cn/main/viewfile/20180108023812835219.html)
- [GmSSL 开源项目](https://github.com/guanzhi/GmSSL)

#### WASM 和 Wasmtime

- [WebAssembly 官方文档](https://webassembly.org/)
- [Wasmtime 文档](https://docs.wasmtime.dev/)
- [WASI 标准](https://wasi.dev/)

#### MCP 协议

- [Model Context Protocol 规范](https://modelcontextprotocol.io/)
- [MCP TypeScript SDK](https://github.com/modelcontextprotocol/typescript-sdk)
- [MCP Python SDK](https://github.com/modelcontextprotocol/python-sdk)

#### Tauri

- [Tauri 官方文档](https://tauri.app/)
- [Tauri 安全指南](https://tauri.app/v1/guides/security/)

#### 数据库

- [PostgreSQL 文档](https://www.postgresql.org/docs/)
- [libSQL 文档](https://github.com/tursodatabase/libsql)
- [ClickHouse 文档](https://clickhouse.com/docs/)
- [TimescaleDB 文档](https://docs.timescale.com/)


### C. 配置示例

#### 管理后台配置

```toml
# config/admin-backend.toml

[server]
host = "0.0.0.0"
port = 8080
workers = 4

[database]
url = "postgresql://user:pass@localhost:5432/admin"
max_connections = 20
min_connections = 5

[crypto]
standard = "ChinaCrypto"  # 或 "Default"
sm2_public_key_path = "/etc/keys/sm2_public.pem"
sm2_private_key_path = "/etc/keys/sm2_private.pem"

[audit]
backend = "ClickHouse"  # 或 "TimescaleDB"
url = "http://localhost:8123"
database = "audit_logs"
batch_size = 100
flush_interval_secs = 10

[skills]
storage_path = "/var/lib/skills"
max_file_size_mb = 100

[clawhub]
api_url = "https://clawhub.io/api"
sync_interval_hours = 24
```

#### 桌面客户端配置

```toml
# config/desktop-client.toml

[backend]
url = "https://admin.example.com"
timeout_secs = 30

[crypto]
standard = "ChinaCrypto"
sm2_public_key_path = "~/.config/app/sm2_public.pem"

[dlp]
cache_path = "~/.local/share/app/dlp_rules.db"
sync_interval_mins = 60

[audit]
local_db_path = "~/.local/share/app/audit.db"
max_offline_logs = 10000
sync_interval_mins = 5

[wasm]
max_memory_mb = 256
max_execution_time_secs = 30
cache_path = "~/.cache/app/wasm"

[ui]
watermark_opacity = 0.15
watermark_text = "{username} ({user_id})"
```

---

## 总结 (Summary)

本设计文档详细描述了企业级 AI Agent 平台的技术架构、核心组件、数据模型、接口设计、安全机制和测试策略。

**核心特性**:
- 国密算法支持，满足政企合规要求
- 多层安全防护，包括 WASM 沙箱、DLP 脱敏、数字签名
- 离线工作能力，支持内网部署
- 完整审计追溯，防篡改存证
- 灵活插件生态，支持 WASM 和 MCP 协议

**技术亮点**:
- Tauri 桌面框架，性能优异
- Rust 全栈，安全可靠
- 属性测试驱动，质量保证
- 灰度发布策略，风险可控

**下一步**:
- 完成 MVP 阶段开发
- 进行安全审计和渗透测试
- 编写用户文档和运维手册
- 准备生产环境部署
