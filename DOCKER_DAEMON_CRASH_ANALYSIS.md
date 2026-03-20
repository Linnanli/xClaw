# Docker Daemon 崩溃问题分析

## 问题现象

执行 `start-all.sh` 后，Docker daemon 断开连接：
1. ✅ 执行前 `docker ps` 正常
2. ✅ 稳定性检查 100% 通过
3. ✅ 脚本开始执行，通过依赖检查
4. ❌ 到"检查 PostgreSQL 数据库"时，Docker daemon 不可用

## 根本原因

**`clean_port 5432` 导致 Docker daemon 崩溃或不稳定**

### 问题代码

```bash
# scripts/start-all.sh (旧版本)
clean_port 5432 "PostgreSQL"

# admin-backend/scripts/common.sh (旧版本)
clean_port() {
    lsof -i :${PORT} | grep -v COMMAND | awk '{print $2}' | xargs kill -9 2>/dev/null || true
}
```

### 为什么会导致问题

1. **5432 是 Docker 容器的端口**
   - PostgreSQL 容器映射 `0.0.0.0:5432->5432/tcp`
   - Docker daemon 管理这个端口映射

2. **`kill -9` 强制杀死进程**
   - 可能杀死 Docker 的端口转发进程
   - 可能杀死 Docker 的网络代理进程
   - 导致 Docker daemon 状态异常

3. **在 macOS 上更严重**
   - Docker Desktop 使用虚拟机
   - 端口转发涉及多个进程
   - 强制杀死可能破坏 Docker 网络栈

### 时间线

```
1. start-all.sh 开始执行
   ↓
2. 检查依赖（Docker 正常）
   ↓
3. 清理端口
   ├─ clean_port 5173 ✅
   ├─ clean_port 38080 ✅
   ├─ clean_port 3000 ✅
   ├─ clean_port 5174 ✅
   └─ clean_port 5432 ❌ 杀死 Docker 相关进程
   ↓
4. Docker daemon 崩溃或进入不稳定状态
   ↓
5. 检查 PostgreSQL 时，docker info 失败
```

## 解决方案

### 方案 1: 不清理容器端口（已实施）

```bash
# scripts/start-all.sh (新版本)
clean_port 5173 "Desktop Client 前端"
clean_port 38080 "Tauri 内嵌后端"
clean_port 3000 "Admin Backend"
clean_port 5174 "Admin Frontend"
# 注意: 不清理 5432 端口，因为这是 Docker 容器的端口
```

**优点**:
- 避免影响 Docker daemon
- 简单直接

**缺点**:
- 如果有其他进程占用 5432，无法清理

### 方案 2: 智能清理（已实施）

```bash
# admin-backend/scripts/common.sh (新版本)
clean_port() {
    local PIDS=$(lsof -ti :${PORT} 2>/dev/null || true)
    
    for PID in $PIDS; do
        local PROCESS_NAME=$(ps -p $PID -o comm= 2>/dev/null || true)
        
        # 跳过 Docker 相关进程
        if echo "$PROCESS_NAME" | grep -qi "docker\|com.docker"; then
            log_warn "跳过 Docker 进程 (PID: $PID)"
            continue
        fi
        
        # 只杀死非 Docker 进程
        kill -9 $PID 2>/dev/null || true
    done
}
```

**优点**:
- 保护 Docker 进程
- 可以清理其他占用端口的进程

**缺点**:
- 稍微复杂一些

## 验证修复

### 测试步骤

```bash
# 1. 确保 Docker 稳定
./scripts/check-docker-stability.sh

# 2. 启动数据库
cd admin-backend && ./scripts/start-db.sh && cd ..

# 3. 验证容器运行
docker ps | grep postgres

# 4. 执行 start-all.sh
./scripts/start-all.sh

# 5. 检查 Docker 是否仍然正常
docker ps
```

### 期望结果

```
✅ 所有应用端口已清理
✅ 检查 PostgreSQL 数据库
✅ PostgreSQL 容器已在运行
✅ PostgreSQL 已就绪
... (其他服务启动)
```

## 经验教训

### 1. 不要清理容器端口

**错误做法**:
```bash
clean_port 5432  # PostgreSQL 容器端口
clean_port 3306  # MySQL 容器端口
clean_port 6379  # Redis 容器端口
```

**正确做法**:
```bash
# 只清理应用层端口
clean_port 3000  # 应用后端
clean_port 5173  # 前端开发服务器
```

### 2. 使用 `kill -9` 要谨慎

**问题**:
- `kill -9` 强制终止，不给进程清理机会
- 可能导致资源泄漏
- 可能破坏进程间通信

**建议**:
- 先尝试 `kill -15` (SIGTERM)
- 等待几秒
- 如果进程仍在，再使用 `kill -9`

### 3. 检查进程名称

**改进前**:
```bash
lsof -i :${PORT} | awk '{print $2}' | xargs kill -9
```

**改进后**:
```bash
# 检查进程名称，跳过 Docker 进程
for PID in $PIDS; do
    PROCESS_NAME=$(ps -p $PID -o comm=)
    if echo "$PROCESS_NAME" | grep -qi "docker"; then
        continue  # 跳过 Docker 进程
    fi
    kill -9 $PID
done
```

### 4. 容器管理应该用 Docker 命令

**错误做法**:
```bash
# 通过杀进程来停止容器
lsof -i :5432 | awk '{print $2}' | xargs kill -9
```

**正确做法**:
```bash
# 使用 Docker 命令管理容器
docker stop admin-backend-postgres
docker rm admin-backend-postgres
```

## 其他可能导致 Docker 崩溃的操作

### 1. 删除 Docker 文件

```bash
# 危险操作
rm -rf ~/.docker/run/docker.sock
rm -rf /var/run/docker.sock
```

### 2. 修改 Docker 配置

```bash
# 危险操作
vim ~/.docker/daemon.json  # 错误的配置可能导致 daemon 无法启动
```

### 3. 资源耗尽

```bash
# 可能导致 Docker 不稳定
docker run --memory=16g ...  # 超过系统内存
docker run --cpus=16 ...     # 超过 CPU 核心数
```

### 4. 网络冲突

```bash
# 可能导致网络问题
docker network create --subnet=172.17.0.0/16 ...  # 与默认网络冲突
```

## 预防措施

### 1. 启动前检查

```bash
# 检查 Docker 稳定性
./scripts/check-docker-stability.sh

# 检查容器状态
docker ps -a

# 检查网络状态
docker network ls
```

### 2. 使用专用脚本

```bash
# 使用专用脚本管理数据库
./admin-backend/scripts/start-db.sh  # 启动
./admin-backend/scripts/stop-db.sh   # 停止（如果有）
```

### 3. 避免直接操作端口

```bash
# 不要直接清理容器端口
# clean_port 5432  # ❌

# 使用 Docker 命令
docker stop admin-backend-postgres  # ✅
```

### 4. 定期清理

```bash
# 定期清理 Docker 资源
docker system prune -f
docker volume prune -f
docker network prune -f
```

## 相关文档

- [DOCKER_ISSUE_SUMMARY.md](DOCKER_ISSUE_SUMMARY.md) - Docker 问题总结
- [DOCKER_STABILITY_ISSUE.md](DOCKER_STABILITY_ISSUE.md) - 稳定性问题诊断
- [STARTUP_QUICK_REFERENCE.md](STARTUP_QUICK_REFERENCE.md) - 启动快速参考
