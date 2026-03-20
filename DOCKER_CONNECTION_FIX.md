# Docker 连接问题修复指南

## 问题症状

执行 `docker ps` 或 `start-all.sh` 时出现错误：

```
Cannot connect to the Docker daemon at unix:///Users/nallylin/.docker/run/docker.sock. 
Is the docker daemon running?
```

## 问题原因

Docker Desktop 应用正在运行，但 Docker Engine（daemon）没有启动。这是 Docker Desktop 的常见问题。

## 快速修复

### 方案 1: 重启 Docker Desktop（推荐）

1. 点击菜单栏的 Docker 图标
2. 选择 **Quit Docker Desktop**
3. 等待 5-10 秒
4. 重新打开 **Docker Desktop**（从 Applications 文件夹）
5. 等待 Docker Engine 启动（菜单栏图标变为正常，不再显示动画）
6. 验证：
   ```bash
   docker ps
   ```

### 方案 2: 使用自动修复脚本

```bash
./scripts/fix-docker-connection.sh
```

脚本会：
- 诊断 Docker 连接问题
- 提供分步修复指引
- 验证修复结果

### 方案 3: 切换 Docker Context

```bash
# 查看可用的 context
docker context ls

# 切换到 default context
docker context use default

# 验证
docker ps

# 如果成功，可以切换回 desktop-linux
docker context use desktop-linux
```

### 方案 4: 重置 Docker Desktop（最后手段）

⚠️ **警告**: 这会删除所有容器、镜像和卷

1. 点击菜单栏的 Docker 图标
2. 选择 **Troubleshoot**
3. 选择 **Reset to factory defaults**
4. 确认重置
5. 等待 Docker Desktop 重启
6. 验证：
   ```bash
   docker ps
   ```

## 验证修复

执行以下命令验证 Docker 是否正常：

```bash
# 1. 检查 Docker 版本（包括 Server）
docker version

# 2. 检查 Docker 信息
docker info

# 3. 列出容器
docker ps

# 4. 列出镜像
docker images
```

如果所有命令都正常执行，说明 Docker 已恢复正常。

## 预防措施

### 1. 确保 Docker Desktop 完全启动

启动 Docker Desktop 后，等待菜单栏图标变为正常状态（不再显示动画或加载图标）。

### 2. 检查 Docker Desktop 设置

打开 Docker Desktop → Settings → General：
- ✅ 确保 "Start Docker Desktop when you log in" 已启用
- ✅ 确保 "Use Docker Compose V2" 已启用

### 3. 定期更新 Docker Desktop

保持 Docker Desktop 为最新版本，避免已知的 bug。

## 常见问题

### Q: 为什么 Docker Desktop 启动了但 daemon 没运行？

A: 可能的原因：
- Docker Desktop 启动过程中出错
- 系统资源不足（内存、磁盘空间）
- Docker Desktop 配置损坏
- macOS 权限问题

### Q: 如何查看 Docker Desktop 日志？

A: 日志位置：
```bash
# Docker Desktop 日志
~/Library/Containers/com.docker.docker/Data/log/

# 查看最新日志
tail -100 ~/Library/Containers/com.docker.docker/Data/log/vm/dockerd.log
```

### Q: 重启后问题依然存在怎么办？

A: 尝试以下步骤：
1. 完全卸载 Docker Desktop
2. 删除配置文件：
   ```bash
   rm -rf ~/Library/Group\ Containers/group.com.docker
   rm -rf ~/Library/Containers/com.docker.docker
   rm -rf ~/.docker
   ```
3. 重新下载并安装 Docker Desktop
4. 重启 macOS

## 相关文档

- [QUICK_START.md](QUICK_START.md) - 快速启动指南
- [admin-backend/TROUBLESHOOTING.md](admin-backend/TROUBLESHOOTING.md) - 完整故障排查
- [Docker Desktop 官方文档](https://docs.docker.com/desktop/troubleshoot/)
