# Cargo 文件锁说明

## 什么是 Cargo 文件锁

Cargo 是 Rust 的包管理器和构建工具。当 Cargo 编译项目时，会创建一个文件锁来防止多个 Cargo 进程同时修改同一个项目。

### 锁文件位置

```
项目根目录/
├── Cargo.lock          # 依赖版本锁定文件
└── target/
    └── .cargo-lock     # 编译过程锁文件
```

## 为什么会有锁冲突

### 场景：start-all.sh 启动流程

```
1. 启动 Tauri 客户端
   ↓
   cargo tauri dev
   ↓
   Cargo 开始编译 Desktop Client
   ↓
   创建文件锁 🔒
   ↓
2. 启动 Admin Backend
   ↓
   cargo run
   ↓
   尝试编译 Admin Backend
   ↓
   检测到文件锁 ❌
   ↓
   错误：另一个 cargo 进程正在使用此项目
```

### 错误信息

```
error: could not lock the package cache
Caused by:
  another cargo process is currently holding the lock
```

或

```
Blocking waiting for file lock on package cache
```

## 解决方案

### 方案 1: 等待 Tauri 编译完成（已实施）

**原理**：等待 Tauri 的初始编译完成后，再启动 Admin Backend

**实现**：
```bash
# 智能等待：检查 Cargo 进程
while [ $WAIT_COUNT -lt $MAX_WAIT ]; do
    if ! pgrep -f "cargo.*tauri" > /dev/null 2>&1; then
        break  # Tauri 编译完成
    fi
    sleep 1
done
```

**优点**：
- ✅ 避免锁冲突
- ✅ 自动检测编译完成
- ✅ 有超时保护

**缺点**：
- ⚠️ 首次编译可能需要等待较长时间

### 方案 2: 使用不同的 target 目录

**原理**：让两个项目使用不同的编译输出目录

**实现**：
```bash
# Desktop Client
CARGO_TARGET_DIR=target/desktop cargo tauri dev

# Admin Backend
CARGO_TARGET_DIR=target/admin cargo run
```

**优点**：
- ✅ 完全避免锁冲突
- ✅ 可以并行编译

**缺点**：
- ❌ 占用更多磁盘空间
- ❌ 编译缓存不共享

### 方案 3: 顺序启动

**原理**：先完全启动一个，再启动另一个

**实现**：
```bash
# 1. 启动并等待 Tauri 完全启动
cargo tauri dev &
wait_for_tauri_ready

# 2. 再启动 Admin Backend
cargo run
```

**优点**：
- ✅ 简单可靠
- ✅ 避免锁冲突

**缺点**：
- ❌ 启动时间较长

## 当前实现

### start-all.sh 中的处理

```bash
# 1. 启动 Tauri 客户端
cargo tauri dev > /tmp/tauri.log 2>&1 &
TAURI_PID=$!

# 2. 等待 Tauri 编译完成
log_info "等待 Tauri 编译完成..."
while [ $WAIT_COUNT -lt $MAX_WAIT ]; do
    if ! pgrep -f "cargo.*tauri" > /dev/null 2>&1; then
        break
    fi
    sleep 1
done

# 3. 启动 Admin Backend
cargo run > /tmp/admin-backend.log 2>&1 &
```

### 等待逻辑

```bash
MAX_WAIT=60  # 最多等待 60 秒

# 检查 Tauri 的 cargo 进程是否还在运行
pgrep -f "cargo.*tauri"

# 如果进程不存在，说明编译完成
```

## 常见问题

### Q1: 为什么要等待？

A: 因为 Cargo 在编译时会锁定项目，防止多个进程同时修改。如果不等待，Admin Backend 的编译会失败。

### Q2: 等待多久？

A: 
- **首次编译**：可能需要 5-10 分钟（下载依赖、编译所有代码）
- **增量编译**：通常 10-30 秒（只编译修改的部分）
- **无修改**：几秒钟（直接启动）

### Q3: 如何加速？

A: 
1. **使用 sccache**：缓存编译结果
   ```bash
   cargo install sccache
   export RUSTC_WRAPPER=sccache
   ```

2. **使用 mold 链接器**：加速链接过程
   ```bash
   brew install mold
   export RUSTFLAGS="-C link-arg=-fuse-ld=mold"
   ```

3. **增加并行编译数**：
   ```bash
   export CARGO_BUILD_JOBS=8
   ```

### Q4: 如果等待超时怎么办？

A: 脚本会继续启动 Admin Backend，但可能会遇到锁冲突。此时：
1. 手动停止 Tauri
2. 等待几秒
3. 重新启动 Admin Backend

### Q5: 可以跳过等待吗？

A: 可以，但不推荐。如果确实需要：
```bash
# 修改 start-all.sh，注释掉等待逻辑
# 或者使用不同的 target 目录
```

## 最佳实践

### 1. 开发时

**推荐**：分别启动，避免锁冲突
```bash
# 终端 1: 启动 Desktop Client
cd desktop-client
cargo tauri dev

# 终端 2: 等待几秒后启动 Admin Backend
cd admin-backend
cargo run
```

### 2. 测试时

**推荐**：使用 start-all.sh，自动处理等待
```bash
./scripts/start-all.sh
```

### 3. 生产环境

**推荐**：预编译，不需要等待
```bash
# 预编译
cargo build --release

# 直接运行二进制文件
./target/release/desktop-client
./target/release/admin-backend
```

## 技术细节

### Cargo 锁机制

Cargo 使用文件系统锁来协调多个进程：

```rust
// Cargo 内部实现（简化）
fn acquire_lock(path: &Path) -> Result<FileLock> {
    let file = File::create(path)?;
    file.lock_exclusive()?;  // 独占锁
    Ok(FileLock { file })
}
```

### 锁的作用范围

- **Package Cache**: `~/.cargo/registry/`
- **Git Cache**: `~/.cargo/git/`
- **Target Directory**: `target/`

### 锁的类型

1. **独占锁（Exclusive Lock）**
   - 编译时使用
   - 只允许一个进程

2. **共享锁（Shared Lock）**
   - 读取时使用
   - 允许多个进程

## 相关命令

### 检查 Cargo 进程

```bash
# 查看所有 cargo 进程
ps aux | grep cargo

# 查看 Tauri 相关的 cargo 进程
pgrep -f "cargo.*tauri"

# 查看进程详情
ps -p <PID> -o pid,ppid,cmd
```

### 清理锁文件

```bash
# 如果遇到锁文件损坏
rm -rf target/.cargo-lock

# 清理整个 target 目录
cargo clean
```

### 强制终止 Cargo

```bash
# 终止所有 cargo 进程
pkill -9 cargo

# 终止特定的 cargo 进程
kill -9 <PID>
```

## 参考资料

- [Cargo Book - Build Cache](https://doc.rust-lang.org/cargo/guide/build-cache.html)
- [Cargo Issue #6930 - File locking](https://github.com/rust-lang/cargo/issues/6930)
- [Rust RFC - Parallel Compilation](https://rust-lang.github.io/rfcs/2349-pin.html)
