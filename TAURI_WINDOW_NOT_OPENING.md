# Tauri 窗口未自动打开问题

## 问题现象

执行 `start-all.sh` 后，Tauri 窗口没有自动打开。

## 常见原因

### 1. 首次编译（最常见）⏳

**现象**：
- 脚本显示"启动 Tauri 客户端"
- 但窗口一直没有出现
- 等待很长时间

**原因**：
Tauri 首次编译需要：
- 下载和编译 Rust 依赖（约 986 个 crate）
- 编译 Tauri 框架
- 编译应用代码

**时间**：
- **首次编译**：10-20 分钟（取决于机器性能）
- **增量编译**：30-60 秒
- **无修改**：5-10 秒

**解决方案**：
```bash
# 查看编译进度
tail -f /tmp/tauri.log

# 你会看到类似：
# Building [============>] 544/986: tauri-utils
# 这表示正在编译第 544 个依赖，总共 986 个
```

**等待编译完成的标志**：
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 15m 23s
```

### 2. 编译错误 ❌

**现象**：
- 编译过程中断
- 窗口没有出现
- 日志中有 `error:` 信息

**检查方法**：
```bash
# 查看错误信息
tail -100 /tmp/tauri.log | grep -i error

# 或查看完整日志
less /tmp/tauri.log
```

**常见错误**：

#### 错误 1: 依赖下载失败
```
error: failed to download from `https://...`
```

**解决**：
```bash
# 清理并重试
cd desktop-client
cargo clean
cargo tauri dev
```

#### 错误 2: 编译失败
```
error[E0425]: cannot find value `xxx` in this scope
```

**解决**：
```bash
# 检查代码错误
# 修复后重新编译
cargo tauri dev
```

#### 错误 3: 前端未启动
```
error: failed to connect to http://localhost:5173
```

**解决**：
```bash
# 确保前端开发服务器运行
cd desktop-client/src-ui
npm run dev
```

### 3. 前端开发服务器未就绪 🌐

**现象**：
- Tauri 编译完成
- 但窗口显示空白或错误

**原因**：
Tauri 配置中 `devUrl: "http://localhost:5173"`，需要前端服务器运行。

**检查方法**：
```bash
# 检查前端是否运行
curl http://localhost:5173

# 检查前端进程
ps aux | grep "vite\|npm run dev" | grep -v grep
```

**解决方案**：
```bash
# 查看前端日志
tail -f /tmp/desktop-frontend.log

# 如果前端未启动，手动启动
cd desktop-client/src-ui
npm run dev
```

### 4. 端口冲突 🔌

**现象**：
- 前端或后端端口被占用
- Tauri 无法连接

**检查方法**：
```bash
# 检查端口占用
lsof -i :5173  # 前端
lsof -i :38080 # 后端
```

**解决方案**：
```bash
# 清理端口（start-all.sh 会自动做）
lsof -i :5173 | grep -v COMMAND | awk '{print $2}' | xargs kill -9
lsof -i :38080 | grep -v COMMAND | awk '{print $2}' | xargs kill -9
```

### 5. macOS 权限问题 🔐

**现象**：
- 编译完成
- 但窗口没有出现
- 没有明显错误

**原因**：
macOS 可能阻止了应用启动。

**检查方法**：
```bash
# 查看系统日志
log show --predicate 'process == "Ironclaw Desktop"' --last 5m
```

**解决方案**：
1. 打开"系统偏好设置" → "安全性与隐私"
2. 查看是否有"Ironclaw Desktop"被阻止
3. 点击"允许"

### 6. 后台运行 🖥️

**现象**：
- 窗口实际已打开
- 但在其他桌面空间或被其他窗口遮挡

**检查方法**：
```bash
# 检查 Tauri 进程
ps aux | grep "Ironclaw Desktop" | grep -v grep

# 或使用 Activity Monitor（活动监视器）
open -a "Activity Monitor"
# 搜索 "Ironclaw"
```

**解决方案**：
- 使用 Mission Control（F3）查看所有窗口
- 或使用 Cmd+Tab 切换应用

## 诊断步骤

### 步骤 1: 检查编译状态

```bash
# 查看 Tauri 日志
tail -f /tmp/tauri.log

# 查找编译进度
# Building [============>] XXX/986
```

**如果看到编译进度**：
- ✅ 正常，等待编译完成
- ⏳ 首次编译需要 10-20 分钟

**如果看到错误**：
- ❌ 查看错误信息
- 🔧 根据错误类型修复

### 步骤 2: 检查前端服务器

```bash
# 检查前端日志
tail -f /tmp/desktop-frontend.log

# 测试前端是否可访问
curl http://localhost:5173
```

**期望输出**：
```html
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    ...
```

### 步骤 3: 检查进程

```bash
# 检查所有相关进程
ps aux | grep -E "tauri|vite|npm" | grep -v grep
```

**期望看到**：
- `cargo-tauri tauri dev`
- `npm run dev` 或 `vite`

### 步骤 4: 手动启动测试

```bash
# 停止 start-all.sh（Ctrl+C）

# 手动启动前端
cd desktop-client/src-ui
npm run dev &

# 等待前端启动（约 5 秒）
sleep 5

# 手动启动 Tauri
cd ../
cargo tauri dev
```

**观察**：
- 是否有错误信息
- 窗口是否出现
- 编译需要多长时间

## 快速修复

### 方案 1: 等待编译完成（推荐）

```bash
# 查看编译进度
tail -f /tmp/tauri.log

# 等待看到：
# Finished `dev` profile [unoptimized + debuginfo] target(s)
```

### 方案 2: 清理并重新编译

```bash
# 停止所有服务
pkill -f "cargo tauri"
pkill -f "npm run dev"

# 清理编译缓存
cd desktop-client
cargo clean

# 重新启动
cd ..
./scripts/start-all.sh
```

### 方案 3: 分步启动

```bash
# 1. 启动前端
cd desktop-client/src-ui
npm run dev &

# 2. 等待前端就绪
sleep 10

# 3. 启动 Tauri（在新终端）
cd ../
cargo tauri dev
```

## 预防措施

### 1. 预编译依赖

```bash
# 首次使用前，预编译 Tauri
cd desktop-client
cargo build

# 这会下载和编译所有依赖
# 之后的启动会快很多
```

### 2. 使用 sccache 加速编译

```bash
# 安装 sccache
cargo install sccache

# 配置环境变量
export RUSTC_WRAPPER=sccache

# 之后的编译会使用缓存
```

### 3. 增加编译并行度

```bash
# 设置并行编译数（根据 CPU 核心数）
export CARGO_BUILD_JOBS=8
```

### 4. 检查系统资源

```bash
# 确保有足够的内存和磁盘空间
# 编译 Tauri 需要：
# - 内存：至少 4GB
# - 磁盘：至少 5GB 空闲空间
```

## 常见问题

### Q: 为什么首次编译这么慢？

A: Tauri 基于 Rust，需要编译约 986 个依赖包。这是一次性的，之后会快很多。

### Q: 如何查看编译进度？

A: 
```bash
tail -f /tmp/tauri.log
```

### Q: 编译失败怎么办？

A:
1. 查看错误信息：`tail -100 /tmp/tauri.log | grep error`
2. 清理并重试：`cargo clean && cargo tauri dev`
3. 检查网络连接（依赖下载需要网络）

### Q: 窗口一直不出现怎么办？

A:
1. 检查编译是否完成（查看日志）
2. 检查前端是否运行（`curl http://localhost:5173`）
3. 检查进程是否存在（`ps aux | grep tauri`）
4. 尝试手动启动（`cd desktop-client && cargo tauri dev`）

### Q: 如何加速后续启动？

A:
1. 使用 sccache 缓存编译结果
2. 不要运行 `cargo clean`
3. 保持依赖版本稳定

## 相关文档

- [CARGO_LOCK_EXPLANATION.md](CARGO_LOCK_EXPLANATION.md) - Cargo 锁机制说明
- [STARTUP_QUICK_REFERENCE.md](STARTUP_QUICK_REFERENCE.md) - 启动快速参考
- [DESKTOP_CLIENT_QUICK_START.md](DESKTOP_CLIENT_QUICK_START.md) - Desktop Client 快速启动
