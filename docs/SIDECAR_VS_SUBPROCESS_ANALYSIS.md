# 子进程自动启动 vs Tauri Sidecar 详解

> 生成日期：2026-03-21
> 目的：详细解释两种方案如何让客户端"双击即用"，不需要用户手动启动 IronClaw 服务。

---

## 当前的问题

```
用户想用客户端：
  1. 手动打开终端                    ← 普通用户不会
  2. 运行 cargo run -- run --no-onboard  ← 普通用户不会
  3. 等待 IronClaw 启动完成           ← 要等 5-15 秒
  4. 再打开桌面客户端                 ← 终于可以用了

期望的体验：
  1. 双击客户端图标                   ← 完事
```

两种方案都能实现"双击即用"，但实现方式不同。

---

## 方案 A：子进程自动启动（std::process::Command）

### 原理

客户端启动时，用 Rust 标准库的 `Command` 在后台启动 IronClaw 二进制文件作为子进程。客户端退出时自动 kill 子进程。

```
┌─────────────────────────────────────────┐
│           操作系统进程树                   │
│                                         │
│  desktop-client (PID 1234)  ← 父进程     │
│    └── ironclaw (PID 1235)  ← 子进程     │
│                                         │
│  父进程退出 → 子进程自动被 kill           │
└─────────────────────────────────────────┘
```

### 实现代码

```rust
use std::process::{Command, Child, Stdio};
use std::time::Duration;
use tokio::time::sleep;

/// IronClaw 子进程管理器
pub struct IronClawProcess {
    child: Option<Child>,
    port: u16,
}

impl IronClawProcess {
    /// 启动 IronClaw 子进程
    pub fn start() -> Result<Self, Box<dyn std::error::Error>> {
        // IronClaw 二进制文件路径
        // 开发环境：从 target 目录找
        // 生产环境：从应用安装目录找（需要打包时一起放进去）
        let ironclaw_path = Self::find_ironclaw_binary()?;
        
        let child = Command::new(&ironclaw_path)
            .args(["run", "--no-onboard", "--cli-only"])
            .stdout(Stdio::piped())   // 捕获标准输出
            .stderr(Stdio::piped())   // 捕获标准错误
            .spawn()?;
        
        println!("✅ IronClaw 子进程已启动 (PID: {})", child.id());
        
        Ok(Self {
            child: Some(child),
            port: 38080,
        })
    }
    
    /// 等待 IronClaw 服务就绪
    pub async fn wait_ready(&self) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!("http://127.0.0.1:{}/api/health", self.port);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()?;
        
        // 最多等待 30 秒
        for i in 0..60 {
            match client.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    println!("✅ IronClaw 服务就绪 (等待了 {}ms)", i * 500);
                    return Ok(());
                }
                _ => {
                    sleep(Duration::from_millis(500)).await;
                }
            }
        }
        
        Err("IronClaw 启动超时（30秒）".into())
    }
    
    /// 停止 IronClaw 子进程
    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
            println!("✅ IronClaw 子进程已停止");
        }
    }
    
    /// 查找 IronClaw 二进制文件
    fn find_ironclaw_binary() -> Result<String, Box<dyn std::error::Error>> {
        // 1. 检查环境变量
        if let Ok(path) = std::env::var("IRONCLAW_BIN") {
            return Ok(path);
        }
        
        // 2. 检查同目录下
        let exe_dir = std::env::current_exe()?
            .parent()
            .unwrap()
            .to_path_buf();
        
        let candidate = exe_dir.join("ironclaw");
        if candidate.exists() {
            return Ok(candidate.to_string_lossy().to_string());
        }
        
        // 3. 检查 PATH
        Err("找不到 ironclaw 二进制文件".into())
    }
}

impl Drop for IronClawProcess {
    fn drop(&mut self) {
        self.stop(); // 客户端退出时自动停止子进程
    }
}
```

### 在 main.rs 中使用

```rust
#[tokio::main]
async fn main() {
    // ... 其他初始化 ...
    
    // 启动 IronClaw 子进程
    let mut ironclaw = IronClawProcess::start()
        .expect("无法启动 IronClaw");
    
    // 等待服务就绪
    ironclaw.wait_ready().await
        .expect("IronClaw 启动超时");
    
    // 启动 Tauri 应用
    tauri::Builder::default()
        .manage(ironclaw)  // 把进程管理器交给 Tauri 管理
        .invoke_handler(tauri::generate_handler![...])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
    
    // Tauri 退出后，ironclaw 的 Drop 会自动 kill 子进程
}
```

### 打包方式

```
安装包结构：
  Ironclaw Desktop.app/          (macOS)
  ├── Contents/
  │   ├── MacOS/
  │   │   ├── desktop-client     ← 客户端二进制
  │   │   └── ironclaw           ← IronClaw 二进制（手动放进去）
  │   └── Resources/
  │       └── ...

  Ironclaw Desktop/              (Windows)
  ├── desktop-client.exe
  └── ironclaw.exe               ← IronClaw 二进制（手动放进去）
```

需要在构建脚本中手动把 IronClaw 二进制复制到打包目录。

### 优缺点

| 维度 | 评价 |
|------|------|
| 实现难度 | 🟢 低（~100 行 Rust 代码） |
| 打包 | 🟡 需要手动处理（构建脚本复制二进制） |
| 跨平台 | 🟡 需要自己处理平台差异（.exe 后缀、路径等） |
| 生命周期管理 | 🟡 基本可用（Drop 自动 kill），但边缘情况需要处理 |
| 进程通信 | 🟡 只能通过 stdout/stderr 或 HTTP |
| Tauri 集成 | ❌ 不是 Tauri 原生方案，需要自己管理 |
| 权限控制 | ❌ 没有 Tauri 的安全沙箱保护 |

### 需要处理的边缘情况

1. **客户端崩溃**：子进程可能变成孤儿进程，需要在下次启动时检测并清理
2. **端口冲突**：38080 端口被占用时需要处理
3. **多实例**：用户打开多个客户端窗口时，不能启动多个 IronClaw
4. **权限问题**：某些系统可能阻止启动子进程
5. **路径问题**：不同平台的二进制路径不同

---

## 方案 C：Tauri Sidecar（推荐）

### 原理

Tauri 框架原生支持"Sidecar"——随应用打包的外部二进制文件。Tauri 负责：
- 自动打包到安装包中
- 自动处理跨平台命名（target triple）
- 提供 API 启动/停止/通信
- 安全权限控制

```
┌─────────────────────────────────────────┐
│           Tauri Sidecar 架构              │
│                                         │
│  Tauri App                              │
│  ├── 前端 (WebView)                     │
│  ├── Rust 后端 (Tauri Commands)          │
│  └── Sidecar: ironclaw                  │
│       ├── 自动打包到安装包               │
│       ├── 自动处理平台命名               │
│       ├── 通过 shell plugin 管理生命周期  │
│       └── 支持 stdin/stdout 通信         │
└─────────────────────────────────────────┘
```

### 第一步：配置 tauri.conf.json

```json
{
  "bundle": {
    "active": true,
    "externalBin": [
      "binaries/ironclaw"
    ]
  }
}
```

### 第二步：准备二进制文件

Tauri 要求 sidecar 二进制文件按平台命名，放在 `src-tauri/binaries/` 目录下：

```
desktop-client/binaries/
├── ironclaw-x86_64-unknown-linux-gnu        ← Linux x86_64
├── ironclaw-aarch64-unknown-linux-gnu       ← Linux ARM64
├── ironclaw-x86_64-apple-darwin             ← macOS Intel
├── ironclaw-aarch64-apple-darwin            ← macOS Apple Silicon
├── ironclaw-x86_64-pc-windows-msvc.exe      ← Windows x86_64
└── ironclaw-aarch64-pc-windows-msvc.exe     ← Windows ARM64
```

可以用构建脚本自动化：

```bash
#!/bin/bash
# scripts/prepare-sidecar.sh

# 获取当前平台的 target triple
TARGET=$(rustc --print host-tuple)

# 编译 IronClaw
cargo build --release -p ironclaw

# 复制到 binaries 目录，加上 target triple 后缀
cp target/release/ironclaw "desktop-client/binaries/ironclaw-${TARGET}"

echo "✅ Sidecar 准备完成: ironclaw-${TARGET}"
```

### 第三步：添加 shell 插件依赖

```toml
# desktop-client/Cargo.toml
[dependencies]
tauri-plugin-shell = "2"
```

### 第四步：配置安全权限

```json
// desktop-client/capabilities/default.json
{
  "identifier": "main-capability",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "shell:allow-spawn",
    "shell:allow-execute",
    {
      "identifier": "shell:allow-spawn",
      "allow": [
        {
          "name": "binaries/ironclaw",
          "sidecar": true,
          "args": [
            "run",
            "--no-onboard",
            "--cli-only",
            { "validator": "\\S+" }
          ]
        }
      ]
    }
  ]
}
```

### 第五步：Rust 端启动 Sidecar

```rust
use tauri::Manager;
use tauri_plugin_shell::ShellExt;
use tauri_plugin_shell::process::CommandEvent;
use std::sync::Arc;
use tokio::sync::Mutex;

/// IronClaw Sidecar 管理器
pub struct IronClawSidecar {
    child: Arc<Mutex<Option<tauri_plugin_shell::process::CommandChild>>>,
}

impl IronClawSidecar {
    pub fn new() -> Self {
        Self {
            child: Arc::new(Mutex::new(None)),
        }
    }
    
    /// 启动 IronClaw sidecar
    pub async fn start(&self, app: &tauri::AppHandle) -> Result<(), String> {
        let sidecar_command = app.shell()
            .sidecar("ironclaw")
            .map_err(|e| format!("无法创建 sidecar 命令: {}", e))?
            .args(["run", "--no-onboard", "--cli-only"]);
        
        let (mut rx, child) = sidecar_command
            .spawn()
            .map_err(|e| format!("无法启动 sidecar: {}", e))?;
        
        // 保存子进程引用
        *self.child.lock().await = Some(child);
        
        // 在后台监听 sidecar 输出
        let app_handle = app.clone();
        tauri::async_runtime::spawn(async move {
            while let Some(event) = rx.recv().await {
                match event {
                    CommandEvent::Stdout(line) => {
                        let msg = String::from_utf8_lossy(&line);
                        tracing::info!("[IronClaw] {}", msg);
                    }
                    CommandEvent::Stderr(line) => {
                        let msg = String::from_utf8_lossy(&line);
                        tracing::warn!("[IronClaw] {}", msg);
                    }
                    CommandEvent::Terminated(payload) => {
                        tracing::error!(
                            "IronClaw 进程退出: code={:?}, signal={:?}",
                            payload.code, payload.signal
                        );
                        // 可以通知前端
                        let _ = app_handle.emit("ironclaw-terminated", payload);
                    }
                    _ => {}
                }
            }
        });
        
        // 等待服务就绪
        self.wait_ready().await?;
        
        Ok(())
    }
    
    /// 等待 IronClaw 服务就绪
    async fn wait_ready(&self) -> Result<(), String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .map_err(|e| e.to_string())?;
        
        for i in 0..60 {
            match client.get("http://127.0.0.1:38080/api/health").send().await {
                Ok(resp) if resp.status().is_success() => {
                    tracing::info!("IronClaw 服务就绪 (等待了 {}ms)", i * 500);
                    return Ok(());
                }
                _ => {
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                }
            }
        }
        
        Err("IronClaw 启动超时（30秒）".to_string())
    }
    
    /// 停止 IronClaw sidecar
    pub async fn stop(&self) -> Result<(), String> {
        if let Some(child) = self.child.lock().await.take() {
            child.kill().map_err(|e| format!("无法停止 sidecar: {}", e))?;
            tracing::info!("IronClaw sidecar 已停止");
        }
        Ok(())
    }
}
```

### 第六步：在 main.rs 中集成

```rust
#[tokio::main]
async fn main() {
    let sidecar = IronClawSidecar::new();
    
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())  // 注册 shell 插件
        .manage(sidecar)
        .setup(|app| {
            // 在应用启动时启动 IronClaw sidecar
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let sidecar = app_handle.state::<IronClawSidecar>();
                match sidecar.start(&app_handle).await {
                    Ok(()) => {
                        tracing::info!("IronClaw sidecar 启动成功");
                        let _ = app_handle.emit("ironclaw-ready", ());
                    }
                    Err(e) => {
                        tracing::error!("IronClaw sidecar 启动失败: {}", e);
                        let _ = app_handle.emit("ironclaw-error", e);
                    }
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            // 窗口关闭时停止 sidecar
            if let tauri::WindowEvent::Destroyed = event {
                let app = window.app_handle().clone();
                tauri::async_runtime::spawn(async move {
                    let sidecar = app.state::<IronClawSidecar>();
                    let _ = sidecar.stop().await;
                });
            }
        })
        .invoke_handler(tauri::generate_handler![...])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### 打包后的结构

Tauri 自动处理打包，最终安装包结构：

```
macOS:
  Ironclaw Desktop.app/
  └── Contents/
      ├── MacOS/
      │   ├── Ironclaw Desktop           ← 客户端
      │   └── ironclaw                   ← Sidecar（Tauri 自动放置）
      └── Resources/
          └── ...

Windows:
  Ironclaw Desktop/
  ├── Ironclaw Desktop.exe               ← 客户端
  └── ironclaw.exe                       ← Sidecar（Tauri 自动放置）

Linux:
  /usr/bin/ironclaw-desktop              ← 客户端
  /usr/bin/ironclaw                      ← Sidecar（Tauri 自动放置）
```

### 优缺点

| 维度 | 评价 |
|------|------|
| 实现难度 | 🟡 中（需要配置 Tauri 插件和权限） |
| 打包 | 🟢 Tauri 自动处理（跨平台命名、打包、安装） |
| 跨平台 | 🟢 Tauri 自动处理（target triple 命名） |
| 生命周期管理 | 🟢 完善（spawn/kill/事件监听） |
| 进程通信 | 🟢 支持 stdin/stdout + HTTP |
| Tauri 集成 | ✅ 原生方案，和 Tauri 生态完美配合 |
| 权限控制 | ✅ Tauri 安全沙箱，可以限制 sidecar 的参数 |

---

## 两种方案对比

### 核心对比表

| 对比维度 | 方案 A：子进程 (std::process::Command) | 方案 C：Tauri Sidecar |
|---------|---------------------------------------|----------------------|
| **实现复杂度** | 🟢 低（~100 行代码） | 🟡 中（~200 行代码 + 配置） |
| **打包自动化** | ❌ 需要手动构建脚本 | ✅ Tauri 自动处理 |
| **跨平台支持** | 🟡 需要自己处理 .exe 后缀、路径差异 | ✅ 自动 target triple 命名 |
| **生命周期管理** | 🟡 基本可用（Drop trait） | ✅ 完善（事件监听、状态回调） |
| **崩溃恢复** | ❌ 需要自己实现孤儿进程检测 | 🟢 Terminated 事件自动通知 |
| **安全权限** | ❌ 无沙箱保护 | ✅ Tauri 权限系统控制 |
| **日志集成** | 🟡 需要手动读取 stdout/stderr | ✅ CommandEvent 自动分发 |
| **前端通知** | ❌ 需要自己实现 | ✅ emit 事件直接通知前端 |
| **多实例防护** | ❌ 需要自己实现（PID 文件等） | 🟢 Tauri 单实例插件配合 |
| **开发体验** | 🟢 简单直接 | 🟡 需要准备 sidecar 二进制 |
| **生产可靠性** | 🟡 中等 | 🟢 高（Tauri 生态验证） |
| **维护成本** | 🟡 需要持续处理平台差异 | 🟢 低（Tauri 框架维护） |

### 生命周期对比

```
方案 A（子进程）：
  客户端启动 → Command::new().spawn() → 轮询 health → 就绪
  客户端退出 → Drop::drop() → child.kill() → 结束
  客户端崩溃 → ⚠️ 子进程可能变成孤儿进程！

方案 C（Sidecar）：
  客户端启动 → shell.sidecar().spawn() → 轮询 health → emit("ready") → 就绪
  客户端退出 → on_window_event → sidecar.stop() → 结束
  客户端崩溃 → Tauri 自动清理 sidecar 进程
  Sidecar 崩溃 → CommandEvent::Terminated → emit("terminated") → 前端提示
```

### 错误处理对比

| 错误场景 | 方案 A 处理方式 | 方案 C 处理方式 |
|---------|---------------|---------------|
| 二进制文件不存在 | `Command::new()` 返回 Err | `sidecar()` 返回 Err |
| 端口被占用 | 需要自己检测 | 需要自己检测 |
| 启动超时 | 轮询 health 超时 | 轮询 health 超时 |
| 运行中崩溃 | 需要自己轮询检测 | Terminated 事件自动通知 |
| 客户端崩溃 | ⚠️ 孤儿进程 | ✅ 自动清理 |
| 权限不足 | 运行时报错 | 编译时权限检查 |

---

## 用户体验流程

### 正常启动流程

```
用户双击客户端图标
  ↓
客户端窗口打开，显示加载界面
  "正在启动 IronClaw 服务..."
  [████████░░░░░░░░] 50%
  ↓
Sidecar/子进程启动 IronClaw
  ↓
轮询 health 接口（每 500ms）
  ↓
IronClaw 就绪（通常 3-10 秒）
  ↓
加载界面消失，进入主界面
  "IronClaw 服务已就绪 ✅"
```

### 异常处理流程

```
场景 1：IronClaw 启动失败
  ↓
超时 30 秒后显示错误
  "IronClaw 服务启动失败"
  [重试] [查看日志] [离线模式]

场景 2：运行中 IronClaw 崩溃
  ↓
前端收到 terminated 事件
  "IronClaw 服务已断开"
  [自动重连中...] 或 [手动重启]

场景 3：端口被占用（已有 IronClaw 运行）
  ↓
检测到 38080 端口已有服务
  直接连接，不启动新进程
  "已连接到现有 IronClaw 服务 ✅"
```

### 前端加载状态组件示例

```typescript
// src-ui/src/components/StartupScreen.tsx
import { listen } from '@tauri-apps/api/event';
import { useState, useEffect } from 'react';

function StartupScreen({ onReady }: { onReady: () => void }) {
  const [status, setStatus] = useState('正在启动 IronClaw 服务...');
  const [progress, setProgress] = useState(0);

  useEffect(() => {
    // 模拟进度
    const timer = setInterval(() => {
      setProgress(p => Math.min(p + 2, 90));
    }, 300);

    // 监听就绪事件
    const unlisten1 = listen('ironclaw-ready', () => {
      setProgress(100);
      setStatus('IronClaw 服务已就绪 ✅');
      setTimeout(onReady, 500);
    });

    // 监听错误事件
    const unlisten2 = listen('ironclaw-error', (event) => {
      setStatus(`启动失败: ${event.payload}`);
      setProgress(0);
    });

    return () => {
      clearInterval(timer);
      unlisten1.then(f => f());
      unlisten2.then(f => f());
    };
  }, [onReady]);

  return (
    <div className="startup-screen">
      <h2>IronClaw Desktop</h2>
      <div className="progress-bar">
        <div className="progress-fill" style={{ width: `${progress}%` }} />
      </div>
      <p>{status}</p>
    </div>
  );
}
```

---

## 崩溃自动重启策略

### 方案 C（Sidecar）的自动重启实现

```rust
/// 带自动重启的 Sidecar 管理器
pub struct ResilientSidecar {
    inner: IronClawSidecar,
    max_restarts: u32,
    restart_count: u32,
    restart_delay: Duration,
}

impl ResilientSidecar {
    pub fn new() -> Self {
        Self {
            inner: IronClawSidecar::new(),
            max_restarts: 3,        // 最多重启 3 次
            restart_count: 0,
            restart_delay: Duration::from_secs(2), // 每次重启间隔 2 秒
        }
    }

    /// 启动并监控，崩溃时自动重启
    pub async fn start_with_monitor(&mut self, app: &tauri::AppHandle) -> Result<(), String> {
        self.inner.start(app).await?;

        let app_handle = app.clone();
        let max_restarts = self.max_restarts;

        // 监听 terminated 事件，自动重启
        app.listen("ironclaw-terminated", move |_event| {
            let app = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                let sidecar = app.state::<ResilientSidecar>();
                // 注意：实际实现需要用 Mutex 保护 restart_count
                tracing::warn!("IronClaw 崩溃，尝试重启...");

                tokio::time::sleep(Duration::from_secs(2)).await;

                match sidecar.inner.start(&app).await {
                    Ok(()) => {
                        tracing::info!("IronClaw 重启成功");
                        let _ = app.emit("ironclaw-ready", ());
                    }
                    Err(e) => {
                        tracing::error!("IronClaw 重启失败: {}", e);
                        let _ = app.emit("ironclaw-error", e);
                    }
                }
            });
        });

        Ok(())
    }
}
```

### 重启策略对比

| 策略 | 方案 A（子进程） | 方案 C（Sidecar） |
|------|----------------|------------------|
| 检测崩溃 | 需要定时轮询 `child.try_wait()` | Terminated 事件自动通知 |
| 重启实现 | 重新 `Command::new().spawn()` | 重新 `sidecar().spawn()` |
| 通知前端 | 需要自己实现通知机制 | `emit()` 直接通知 |
| 重启次数限制 | 需要自己实现 | 需要自己实现 |
| 指数退避 | 需要自己实现 | 需要自己实现 |

---

## 开发环境 vs 生产环境

### 开发环境

```
开发时的工作流：

方案 A（子进程）：
  1. 先编译 IronClaw: cargo build -p ironclaw
  2. 设置环境变量: export IRONCLAW_BIN=./target/debug/ironclaw
  3. 运行客户端: cargo tauri dev
  → 客户端自动启动 IronClaw 子进程

方案 C（Sidecar）：
  1. 先编译 IronClaw: cargo build -p ironclaw
  2. 复制到 binaries 目录: ./scripts/prepare-sidecar.sh
  3. 运行客户端: cargo tauri dev
  → Tauri 自动从 binaries/ 找到 sidecar 并启动

  或者（更方便的开发模式）：
  1. 手动启动 IronClaw: cargo run -p ironclaw -- run --no-onboard
  2. 运行客户端: cargo tauri dev
  → 客户端检测到 38080 端口已有服务，跳过 sidecar 启动
```

### 生产环境

```
生产打包流程：

方案 A（子进程）：
  1. cargo build --release -p ironclaw
  2. cargo build --release -p desktop-client
  3. 手动编写打包脚本，把两个二进制放在一起
  4. 用 tauri build 打包（需要自定义 beforeBuildCommand）

方案 C（Sidecar）：
  1. cargo build --release -p ironclaw
  2. ./scripts/prepare-sidecar.sh  （复制到 binaries/ 并重命名）
  3. cargo tauri build
  → Tauri 自动把 sidecar 打包进安装包
```

### 开发便利性对比

| 维度 | 方案 A | 方案 C |
|------|--------|--------|
| 首次配置 | 🟢 几乎无配置 | 🟡 需要配置 tauri.conf.json + 权限 |
| 日常开发 | 🟢 设置环境变量即可 | 🟡 需要 prepare-sidecar 脚本 |
| 独立调试 | 🟢 可以单独运行 IronClaw | 🟢 可以单独运行 IronClaw |
| CI/CD 集成 | 🟡 需要自定义打包步骤 | 🟢 `cargo tauri build` 一步到位 |
| 跨平台构建 | ❌ 需要为每个平台写脚本 | ✅ Tauri 自动处理 |

---

## 最终推荐

### 推荐方案：Tauri Sidecar（方案 C）

**理由**：

1. **Tauri 原生方案**：既然已经选择了 Tauri 框架，使用其原生的 Sidecar 机制是最自然的选择。不需要重新发明轮子。

2. **打包自动化**：生产环境打包是最大的痛点。Sidecar 方案让 Tauri 自动处理跨平台二进制命名、打包、安装，省去大量构建脚本维护。

3. **生命周期管理完善**：Terminated 事件、自动清理、前端通知——这些在子进程方案中都需要自己实现，Sidecar 开箱即用。

4. **安全性**：Tauri 的权限系统可以限制 sidecar 的启动参数，防止恶意调用。

5. **社区支持**：Tauri Sidecar 是官方推荐的方案，有完善的文档和社区支持。遇到问题更容易找到解决方案。

### 什么时候用方案 A（子进程）？

- 快速原型验证阶段，不想花时间配置 Sidecar
- 非 Tauri 项目（纯 Rust GUI 或 Electron 等）
- 需要极简实现，不关心打包自动化

### 实施路线图

```
第 1 步：开发环境验证（1 天）
  ├── 配置 tauri.conf.json 的 externalBin
  ├── 准备 sidecar 二进制（prepare-sidecar.sh）
  ├── 实现 IronClawSidecar 管理器
  └── 验证 cargo tauri dev 能自动启动 IronClaw

第 2 步：前端集成（0.5 天）
  ├── 实现 StartupScreen 加载组件
  ├── 监听 ironclaw-ready / ironclaw-error 事件
  └── 处理启动失败的用户提示

第 3 步：崩溃恢复（0.5 天）
  ├── 实现自动重启逻辑（最多 3 次）
  ├── 实现指数退避
  └── 前端断线重连提示

第 4 步：生产打包验证（1 天）
  ├── cargo tauri build 生成安装包
  ├── 验证安装包中包含 sidecar
  ├── 在干净环境测试安装和启动
  └── 跨平台验证（macOS / Windows / Linux）

总计：约 3 天
```

---

## 总结

| | 方案 A：子进程 | 方案 C：Tauri Sidecar |
|--|---------------|----------------------|
| 一句话描述 | 用 Rust 标准库手动管理 IronClaw 进程 | 用 Tauri 框架原生机制管理 IronClaw 进程 |
| 适合场景 | 快速原型、非 Tauri 项目 | 生产级 Tauri 应用（推荐） |
| 实现成本 | 低（但后续维护成本高） | 中（但后续维护成本低） |
| 核心优势 | 简单直接 | 自动化打包 + 完善的生命周期管理 |
| 核心劣势 | 打包和跨平台需要大量手动处理 | 初始配置稍复杂 |

> **结论**：对于 IronClaw Desktop Client 这样的 Tauri 应用，Tauri Sidecar 是正确的选择。初始配置多花半天时间，换来的是长期的打包自动化、可靠的进程管理、和更好的用户体验。


---

## 附录：Sidecar 打包膨胀问题分析

> 这是一个关键问题：无论方案 A 还是方案 C，都需要把 IronClaw 二进制打包进客户端。但 IronClaw 是一个全功能单体二进制，包含了大量客户端根本不需要的能力。

### IronClaw 二进制包含的全部模块

从 `ironclaw/src/lib.rs` 和 `Cargo.toml` 分析，IronClaw 二进制包含：

```
ironclaw 二进制（debug: 210MB，release 预估: 60-80MB）
│
├── 🟢 客户端需要的（约 30%）
│   ├── agent/          - AI Agent 核心（对话、推理）
│   ├── llm/            - LLM 后端（OpenAI 等）
│   ├── skills/         - 技能系统
│   ├── extensions/     - 扩展管理（MCP）
│   ├── settings/       - 设置管理
│   ├── safety/         - 安全过滤
│   ├── config/         - 配置管理
│   ├── db/             - 数据库（libSQL 部分）
│   ├── secrets/        - 密钥管理
│   └── channels/web/   - Web Gateway API（客户端通过 HTTP 调用）
│
├── 🟡 客户端可能需要的（约 20%）
│   ├── tools/          - 内置工具（文件、搜索等）
│   ├── workspace/      - 工作空间/记忆
│   ├── history/        - 对话历史
│   ├── context/        - 上下文管理
│   ├── hooks/          - 生命周期钩子
│   └── estimation/     - 成本估算
│
├── 🔴 客户端完全不需要的（约 50%）
│   ├── channels/signal/     - Signal 通道
│   ├── channels/http/       - HTTP Webhook 通道
│   ├── channels/wasm/       - WASM 通道（Telegram, Slack, Discord 等）
│   ├── channels/relay/      - 中继通道
│   ├── channels/repl/       - CLI 交互通道
│   ├── cli/                 - 完整的 CLI 命令系统（20+ 子命令）
│   ├── sandbox/             - Docker 沙箱（依赖 bollard）
│   ├── worker/              - 后台 Worker 进程
│   ├── orchestrator/        - 任务编排器
│   ├── pairing/             - 设备配对
│   ├── tunnel/              - 隧道（ngrok 等）
│   ├── webhooks/            - Webhook 服务器
│   ├── import/              - 数据导入
│   ├── document_extraction/ - PDF/文档提取
│   ├── transcription/       - 语音转文字
│   ├── evaluation/          - 评估系统
│   ├── observability/       - 可观测性
│   ├── boot_screen/         - CLI 启动画面
│   └── setup/               - 安装向导
│
└── 🔴 客户端不需要的重量级依赖
    ├── wasmtime          - WASM 运行时（~15MB）
    ├── bollard            - Docker 客户端（~5MB）
    ├── deadpool-postgres  - PostgreSQL 连接池
    ├── tokio-postgres     - PostgreSQL 驱动
    ├── pdf-extract        - PDF 解析
    ├── crossterm/rustyline - 终端 UI
    └── ed25519-dalek      - 签名验证
```

### 具体的"浪费"

| 不需要的能力 | 对应依赖 | 预估大小 |
|-------------|---------|---------|
| WASM 通道（Telegram/Slack/Discord） | wasmtime, wasmparser, wasmtime-wasi | ~15MB |
| Docker 沙箱 | bollard, hyper, flate2, tar | ~8MB |
| PostgreSQL 支持 | deadpool-postgres, tokio-postgres, refinery | ~5MB |
| CLI 系统 | clap, crossterm, rustyline, termimad | ~3MB |
| PDF/文档提取 | pdf-extract, zip | ~3MB |
| Signal/HTTP/Relay 通道 | 各种通道实现 | ~2MB |
| 隧道/配对/导入 | 各种辅助功能 | ~2MB |
| **合计浪费** | | **~38MB（release 二进制的约 50%）** |

### 这意味着什么？

```
用户安装 IronClaw Desktop：
  ├── desktop-client 二进制    ~15MB
  └── ironclaw sidecar 二进制  ~70MB（release + strip + LTO）
      ├── 客户端需要的部分      ~35MB（50%）
      └── 客户端不需要的部分    ~35MB（50%）← 浪费

总安装包大小：~85MB
其中浪费：~35MB
```

### 解决方案分析

#### 方案 1：接受全量打包（推荐短期方案）

**做法**：直接把完整的 IronClaw 二进制作为 Sidecar 打包。

**理由**：
- 70-85MB 的安装包在 2026 年是完全可接受的（VS Code ~300MB, Slack ~200MB, Discord ~150MB）
- 不需要修改 IronClaw 代码
- 立即可用，零额外开发成本
- IronClaw 的 `--cli-only` 参数已经可以禁用不需要的通道

**trade-off**：
```
优点：
  ✅ 零开发成本
  ✅ 立即可用
  ✅ IronClaw 更新时客户端自动获得所有新功能
  ✅ 安装包大小在可接受范围内

缺点：
  ❌ 安装包比理想情况大 ~35MB
  ❌ 内存中加载了不需要的代码（但未执行的代码不占太多内存）
  ❌ 启动时间可能略长（加载更大的二进制）
```

#### 方案 2：IronClaw 添加 Feature Flags（推荐长期方案）

**做法**：在 IronClaw 的 `Cargo.toml` 中添加 feature flags，让客户端可以编译一个精简版。

```toml
# ironclaw/Cargo.toml（改造后）
[features]
default = ["full"]
full = ["postgres", "libsql", "html-to-markdown", "wasm-channels", "docker-sandbox", "cli-ui", "signal-channel"]

# 客户端只需要这些
client = ["libsql", "gateway-api"]

# 可选模块
wasm-channels = ["dep:wasmtime", "dep:wasmtime-wasi", "dep:wasmparser"]
docker-sandbox = ["dep:bollard"]
cli-ui = ["dep:crossterm", "dep:rustyline", "dep:termimad"]
signal-channel = []
gateway-api = []  # Web Gateway API（客户端必需）
```

**编译精简版**：
```bash
# 完整版（服务器部署）
cargo build --release -p ironclaw

# 精简版（客户端 Sidecar）
cargo build --release -p ironclaw --no-default-features --features client
```

**预估效果**：
```
完整版二进制：~70MB
精简版二进制：~35-40MB（减少 40-50%）
```

**trade-off**：
```
优点：
  ✅ 安装包减小 30-35MB
  ✅ 启动更快
  ✅ 攻击面更小（没有 Docker、WASM 等）

缺点：
  ❌ 需要修改 IronClaw 代码（大量 #[cfg(feature = "...")] 注解）
  ❌ 开发成本高（预估 2-3 周）
  ❌ 增加 CI/CD 复杂度（需要编译两个版本）
  ❌ 可能引入条件编译 bug
```

#### 方案 3：IronClaw 拆分为 lib + 多个 bin（理想但成本最高）

**做法**：把 IronClaw 拆分为核心库 + 多个二进制。

```
ironclaw/
├── crates/
│   ├── ironclaw-core/     ← 核心库（agent, llm, db, safety）
│   ├── ironclaw-gateway/  ← Web Gateway 服务
│   ├── ironclaw-cli/      ← CLI 工具
│   └── ironclaw-worker/   ← 后台 Worker
├── src/
│   └── main.rs            ← 全功能二进制（组合所有 crates）
└── Cargo.toml
```

客户端只需要打包 `ironclaw-gateway`（~30MB）。

**trade-off**：
```
优点：
  ✅ 最小的安装包
  ✅ 最清晰的架构
  ✅ 各组件独立部署

缺点：
  ❌ 需要大规模重构 IronClaw（预估 1-2 个月）
  ❌ 可能破坏现有的 CI/CD 和发布流程
  ❌ 需要 IronClaw 团队配合
```

### 推荐策略

```
短期（现在）：方案 1 - 接受全量打包
  → 直接用完整 IronClaw 二进制作为 Sidecar
  → 安装包 ~85MB，完全可接受
  → 零额外开发成本

中期（3-6 个月后）：方案 2 - Feature Flags
  → 当用户量增长，安装包大小成为关注点时
  → 为 IronClaw 添加 feature flags
  → 编译精简版 Sidecar（~40MB）

长期（1 年后）：方案 3 - 架构拆分
  → 当 IronClaw 本身需要微服务化时
  → 拆分为核心库 + 多个独立服务
  → 客户端只打包 gateway 服务
```

### 关于"管理端能力也打进来了"的具体回答

是的，你说得对。Sidecar 方案会把以下管理端/服务端能力也打包进客户端：

| 不需要的管理端能力 | 说明 |
|------------------|------|
| Telegram/Slack/Discord 通道 | 这些是服务端的消息通道，客户端不需要 |
| Docker 沙箱 | 服务端用来隔离执行代码，客户端不需要 |
| Worker 进程 | 服务端的后台任务执行器 |
| Orchestrator | 服务端的任务编排 |
| PostgreSQL 支持 | 服务端用的数据库，客户端用 libSQL |
| CLI 系统 | 服务端的命令行工具 |
| 隧道/配对 | 服务端的网络功能 |

但这些代码虽然被编译进了二进制，在客户端运行时并不会被执行（因为用 `--cli-only` 启动，这些通道不会被激活）。所以：

- **磁盘空间**：多占 ~35MB（可接受）
- **内存**：未执行的代码不占运行时内存（操作系统按需加载页面）
- **安全性**：代码存在但不执行，攻击面略大但风险很低
- **启动时间**：几乎无影响（二进制加载是 mmap，不是全量读取）

**结论**：短期内直接全量打包是最务实的选择。35MB 的"浪费"换来的是零开发成本和立即可用。
