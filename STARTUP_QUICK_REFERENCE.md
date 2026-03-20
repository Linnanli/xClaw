# 启动快速参考

## ✅ Docker 稳定后的正确启动流程

### 1. 确保 Docker 稳定（首次或遇到问题时）

```bash
# 完全重启 Docker Desktop
osascript -e 'quit app "Docker"'
sleep 10
open -a Docker
sleep 60

# 检查稳定性
./scripts/check-docker-stability.sh
```

期望输出：
```
✅ Docker daemon 运行稳定 ✨
成功: 10/10 (100%)
```

### 2. 启动数据库

```bash
cd admin-backend
./scripts/start-db.sh
cd ..
```

期望输出：
```
✅ PostgreSQL 容器已在运行
✅ PostgreSQL 已就绪
```

### 3. 启动所有服务

#### 并行模式（默认，快速）

```bash
./scripts/start-all.sh
```

期望输出：
```
✅ 检查 PostgreSQL 数据库
✅ PostgreSQL 容器已在运行
✅ PostgreSQL 已就绪
... (其他服务启动)
🎉 所有服务启动完成
```

#### 串行模式（推荐首次运行，可看到编译进度）

```bash
./scripts/start-all.sh --serial
# 或
./scripts/start-all.sh -s
```

期望输出：
```
✅ 检查 PostgreSQL 数据库
✅ PostgreSQL 容器已在运行
✅ PostgreSQL 已就绪

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
启动 Tauri 客户端（串行模式 - 实时显示编译进度）
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

✅ 启动 Tauri 客户端（内嵌 IronClaw 核心服务）...
✅ 内嵌后端将在端口 38080 启动

✅ Tauri 客户端进程 PID: 12345
✅ 正在编译，实时显示进度...

   Compiling ironclaw v0.1.0
   Compiling desktop-client v0.1.0
   ...
   Finished dev [unoptimized + debuginfo] target(s) in 2m 30s

✅ Tauri 客户端和内嵌后端已启动

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
启动 Admin Backend 后端（串行模式 - 实时显示编译进度）
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

✅ 启动后端...

✅ 后端进程 PID: 12346
✅ 正在编译，实时显示进度...

   Compiling admin-backend v0.1.0
   ...
   Finished dev [unoptimized + debuginfo] target(s) in 1m 15s

✅ 后端编译完成并启动成功

🎉 所有服务启动完成
```

**串行模式优点**：
- 实时看到每个服务的编译进度
- 清楚知道当前在编译哪个服务
- 首次运行推荐使用（编译时间长）

**并行模式优点**：
- 启动速度更快（服务并行启动）
- 适合后续运行（增量编译快）

## 🔧 脚本说明

### 数据库相关

| 脚本 | 用途 | 行为 |
|------|------|------|
| `admin-backend/scripts/start-db.sh` | 启动数据库 | 自动启动 PostgreSQL 容器 |
| `start-all.sh` 中的检查 | 检查数据库 | 仅检查，不启动 |

### Docker 诊断

| 脚本 | 用途 |
|------|------|
| `scripts/check-docker-stability.sh` | 检查 Docker daemon 稳定性 |
| `scripts/fix-docker-connection.sh` | 自动诊断和修复 Docker 连接 |

## 📋 常见场景

### 场景 1: 首次启动（推荐串行模式）

```bash
# 1. 检查 Docker 稳定性
./scripts/check-docker-stability.sh

# 2. 启动数据库
cd admin-backend && ./scripts/start-db.sh && cd ..

# 3. 启动所有服务（串行模式，可看到编译进度）
./scripts/start-all.sh --serial
```

### 场景 2: 数据库已运行，只启动应用（并行模式）

```bash
# 直接启动所有服务（会检查数据库）
./scripts/start-all.sh
```

### 场景 3: Docker 不稳定

```bash
# 1. 完全重启 Docker
osascript -e 'quit app "Docker"'
sleep 10
open -a Docker
sleep 60

# 2. 检查稳定性
./scripts/check-docker-stability.sh

# 3. 如果稳定，继续启动
cd admin-backend && ./scripts/start-db.sh && cd ..
./scripts/start-all.sh
```

### 场景 4: 遇到错误

```bash
# 运行自动诊断
./scripts/fix-docker-connection.sh

# 按照提示操作
```

## ⚠️ 常见错误

### 错误 1: `start_postgres_db: command not found`

**原因**: 旧版本的 `common.sh`

**解决**: 已修复，重新运行即可

### 错误 2: `PostgreSQL 容器未运行`

**原因**: 数据库未启动

**解决**:
```bash
cd admin-backend
./scripts/start-db.sh
cd ..
```

### 错误 3: `Docker daemon 不可用`

**原因**: Docker 不稳定

**解决**:
```bash
osascript -e 'quit app "Docker"'
sleep 10
open -a Docker
sleep 60
./scripts/check-docker-stability.sh
```

## 🎯 关键点

1. **Docker 必须稳定** - 使用 `check-docker-stability.sh` 验证
2. **数据库必须先启动** - 使用 `start-db.sh` 启动
3. **start-all.sh 只检查数据库** - 不会自动启动数据库
4. **首次运行推荐串行模式** - 可以看到编译进度，了解当前状态
5. **后续运行使用并行模式** - 增量编译快，并行启动更高效

## 🔄 启动模式对比

| 特性 | 并行模式（默认） | 串行模式（--serial） |
|------|-----------------|---------------------|
| 启动速度 | 快（服务并行启动） | 慢（逐个启动） |
| 编译进度 | 不可见（后台编译） | 实时显示 |
| 适用场景 | 后续运行、增量编译 | 首次运行、完整编译 |
| 命令 | `./scripts/start-all.sh` | `./scripts/start-all.sh --serial` |
| 性能影响 | 无（并行不影响性能） | 无（串行只是等待，不影响编译性能） |

## 📚 相关文档

- [QUICK_START.md](QUICK_START.md) - 完整启动指南
- [DOCKER_ISSUE_SUMMARY.md](DOCKER_ISSUE_SUMMARY.md) - Docker 问题总结
- [IMMEDIATE_FIX.md](IMMEDIATE_FIX.md) - 立即修复指南
