# Docker 启动问题修复总结

**问题时间**: 2026-03-20  
**问题**: PostgreSQL 启动超时  
**根本原因**: Docker daemon 未运行

---

## 问题现象

运行 `start-admin.sh` 或 `start-db.sh` 时出现:

```
✅ 等待 PostgreSQL 启动...
数据库启动进度: ..............................
❌ PostgreSQL 启动超时
查看日志: docker logs admin-backend-postgres
```

运行 `docker logs admin-backend-postgres` 时出现:

```
Cannot connect to the Docker daemon at unix:///Users/nallylin/.docker/run/docker.sock. 
Is the docker daemon running?
```

---

## 根本原因

Docker Desktop 应用未启动,导致 Docker daemon 无法连接。

---

## 解决方案

### 立即修复

1. **启动 Docker Desktop**
   - **macOS**: 打开 Launchpad 或 Applications 文件夹,点击 Docker 应用
   - **Windows**: 打开开始菜单,搜索并启动 "Docker Desktop"
   - **Linux**: `sudo systemctl start docker`

2. **等待 Docker 启动**
   - 菜单栏/系统托盘会出现 Docker 图标
   - 图标变为绿色表示 Docker 已就绪

3. **验证 Docker 运行**
   ```bash
   docker ps
   ```
   应该能看到容器列表(可能为空)

4. **重新运行启动脚本**
   ```bash
   cd admin-backend
   ./scripts/start-admin.sh
   ```

### 使用诊断脚本

我们创建了一个新的诊断脚本来快速检查所有服务状态:

```bash
cd admin-backend
./scripts/diagnose.sh
```

诊断脚本会检查:
- ✅ Docker 是否安装
- ✅ Docker daemon 是否运行
- ✅ PostgreSQL 容器状态
- ✅ 后端服务状态
- ✅ 前端服务状态
- ✅ Rust 和 Node.js 环境
- ✅ 日志文件位置

并提供针对性的修复建议。

---

## 已完成的改进

### 1. 更新 `common.sh` 脚本

**文件**: `admin-backend/scripts/common.sh`

**改进**: 在 `check_docker_dependencies()` 函数中添加 Docker daemon 运行检查

**新增代码**:
```bash
# 检查 Docker daemon 是否运行
if ! docker ps &> /dev/null; then
    log_error "Docker daemon 未运行"
    echo ""
    echo "请启动 Docker Desktop:"
    echo "  macOS: 打开 Applications 文件夹，启动 Docker 应用"
    echo "  Windows: 打开开始菜单，搜索并启动 Docker Desktop"
    echo "  Linux: sudo systemctl start docker"
    echo ""
    exit 1
fi
log_info "Docker daemon 正在运行"
```

**效果**: 脚本会在启动前检查 Docker daemon,如果未运行会立即提示用户,而不是等待 30 秒超时。

---

### 2. 更新故障排除文档

**文件**: `admin-backend/TROUBLESHOOTING.md`

**改进**: 在常见错误列表最前面添加 "Docker 未运行" 问题

**新增章节**:
```markdown
### 0. Docker 未运行 🔴

**错误信息**:
- Cannot connect to the Docker daemon
- ❌ PostgreSQL 启动超时

**原因**: Docker Desktop 应用未启动

**解决方案**: 启动 Docker Desktop
```

---

### 3. 创建诊断脚本

**文件**: `admin-backend/scripts/diagnose.sh`

**功能**:
- 检查 Docker 安装和运行状态
- 检查 PostgreSQL 容器状态
- 检查后端服务状态 (端口 3000)
- 检查前端服务状态 (端口 5174)
- 检查 Rust 和 Node.js 环境
- 检查日志文件位置
- 提供针对性的修复建议

**使用方法**:
```bash
cd admin-backend
./scripts/diagnose.sh
```

**输出示例**:
```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Admin Backend 诊断工具
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

[1/7] 检查 Docker
✅ Docker 已安装
❌ Docker daemon 未运行
   解决方案: 启动 Docker Desktop 应用

[2/7] 检查 PostgreSQL 容器
❌ PostgreSQL 容器未运行
   解决方案: cd admin-backend && ./scripts/start-db.sh

...

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
诊断完成
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

快速修复建议:

1. 启动 Docker Desktop
   macOS: 打开 Applications 文件夹，启动 Docker 应用
   Windows: 打开开始菜单，搜索并启动 Docker Desktop
```

---

### 4. 更新脚本 README

**文件**: `admin-backend/scripts/README.md`

**改进**: 在"常见问题解决"部分最前面添加:
- 快速诊断章节
- Docker 未运行问题

---

## 预防措施

### 开发前检查清单

在运行启动脚本前,确保:

- [ ] Docker Desktop 已启动(菜单栏/系统托盘有绿色图标)
- [ ] 可以运行 `docker ps` 命令
- [ ] 端口 3000、5174、5432 未被占用

### 使用诊断脚本

遇到任何问题时,首先运行:

```bash
cd admin-backend
./scripts/diagnose.sh
```

诊断脚本会告诉你:
- 哪些服务正常运行 ✅
- 哪些服务有问题 ❌
- 如何修复问题 💡

---

## 相关文档

- `admin-backend/TROUBLESHOOTING.md` - 完整的故障排查指南
- `admin-backend/scripts/README.md` - 脚本使用说明
- `admin-backend/scripts/diagnose.sh` - 诊断脚本
- `admin-backend/scripts/common.sh` - 共享函数库

---

## 总结

**问题**: PostgreSQL 启动超时  
**原因**: Docker daemon 未运行  
**解决**: 启动 Docker Desktop

**改进**:
1. ✅ 添加 Docker daemon 运行检查
2. ✅ 更新故障排除文档
3. ✅ 创建诊断脚本
4. ✅ 更新脚本 README

**下次遇到问题**:
```bash
cd admin-backend
./scripts/diagnose.sh
```

---

**修复时间**: 2026-03-20  
**修复人员**: Kiro AI Assistant
