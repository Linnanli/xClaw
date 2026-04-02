# Admin Backend 故障排查指南

## 快速诊断

### 检查所有服务状态

```bash
# 检查数据库
docker ps | grep admin-backend-postgres

# 检查后端
lsof -i :3000

# 检查前端
lsof -i :5174

# 查看所有日志
tail -f /tmp/admin-backend.log
tail -f /tmp/admin-frontend.log
docker logs admin-backend-postgres
```

---

## 常见错误及解决方案

### 0. Docker 未运行 🔴

**错误信息**:
```
Cannot connect to the Docker daemon at unix:///var/run/docker.sock. Is the docker daemon running?
```

或

```
❌ PostgreSQL 启动超时
查看日志: docker logs admin-backend-postgres
```

**原因**: Docker Desktop 应用未启动

**诊断步骤**:
```bash
# 检查 Docker 是否运行
docker ps

# 如果看到 "Cannot connect to the Docker daemon"，说明 Docker 未运行
```

**解决方案**:

**macOS**:
1. 打开 Launchpad 或 Applications 文件夹
2. 找到并点击 "Docker" 应用
3. 等待 Docker Desktop 启动（菜单栏会出现 Docker 图标）
4. 确认 Docker 图标显示为绿色（运行中）
5. 重新运行启动脚本

**Windows**:
1. 打开开始菜单
2. 搜索并启动 "Docker Desktop"
3. 等待 Docker Desktop 启动
4. 确认系统托盘中的 Docker 图标显示为绿色
5. 重新运行启动脚本

**Linux**:
```bash
# 启动 Docker 服务
sudo systemctl start docker

# 设置开机自启
sudo systemctl enable docker

# 检查状态
sudo systemctl status docker
```

**验证修复**:
```bash
# 应该能看到 Docker 容器列表（可能为空）
docker ps

# 应该能看到 Docker 版本信息
docker --version
```

---

### 1. 数据库连接错误 ⚠️

**错误信息**:
```
Database error: Error occurred while creating a new object: error connecting to server
```

或

```
api/dlp-rules 接口报错 500:
{"details": "Database error: Error occurred while creating a new object: error connecting to server","error": "Database error"}
```

**原因**: PostgreSQL 数据库未启动

**诊断步骤**:
```bash
# 1. 检查数据库容器是否运行
docker ps | grep admin-backend-postgres

# 2. 如果没有输出，说明数据库未启动
```

**解决方案**:

**方案 1: 使用完整启动脚本（推荐）**
```bash
cd admin-backend
./scripts/start-admin.sh
```

**方案 2: 单独启动数据库**
```bash
cd admin-backend
./scripts/start-db.sh
```

**方案 3: 手动启动数据库**
```bash
cd admin-backend
docker-compose up -d postgres

# 等待数据库就绪
docker exec admin-backend-postgres pg_isready -U postgres
```

**验证修复**:
```bash
# 1. 确认数据库运行
docker ps | grep admin-backend-postgres

# 2. 测试连接
docker exec admin-backend-postgres pg_isready -U postgres

# 3. 重启后端
cd admin-backend
cargo run
```

---

### 2. 端口被占用

**错误信息**:
```
Error: Address already in use (os error 48)
```

**诊断步骤**:
```bash
# 检查哪个进程占用了端口
lsof -i :3000  # 后端
lsof -i :5174  # 前端
lsof -i :5432  # 数据库
```

**解决方案**:

**清理 3000 端口（后端）**:
```bash
lsof -i :3000 | grep -v COMMAND | awk '{print $2}' | xargs kill -9
```

**清理 5174 端口（前端）**:
```bash
lsof -i :5174 | grep -v COMMAND | awk '{print $2}' | xargs kill -9
```

**清理 5432 端口（数据库）**:
```bash
cd admin-backend
docker-compose down
```

**清理所有端口**:
```bash
# 停止所有相关进程
pkill -f "admin-backend"
pkill -f "ironclaw.*run"

# 清理端口
lsof -i :3000 | grep -v COMMAND | awk '{print $2}' | xargs kill -9
lsof -i :5174 | grep -v COMMAND | awk '{print $2}' | xargs kill -9

# 停止数据库
cd admin-backend
docker-compose down
```

---

### 3. 后端编译失败

**错误信息**:
```
error: could not compile `admin-backend`
```

**诊断步骤**:
```bash
# 查看完整编译日志
tail -100 /tmp/admin-backend.log

# 或手动编译查看错误
cd admin-backend
cargo build
```

**常见原因及解决方案**:

**原因 1: 依赖未安装**
```bash
cd admin-backend
cargo build
```

**原因 2: 数据库未启动**
```bash
./scripts/start-db.sh
```

**原因 3: 环境变量缺失**
```bash
# 检查 .env 文件
cat admin-backend/.env

# 如果不存在，复制示例文件
cp admin-backend/.env.example admin-backend/.env
```

**原因 4: Rust 版本过旧**
```bash
rustup update
```

---

### 4. 前端启动失败

**错误信息**:
```
Error: Cannot find module 'vite'
```

**诊断步骤**:
```bash
# 查看前端日志
tail -100 /tmp/admin-frontend.log

# 检查 node_modules
ls admin-backend/ui/node_modules
```

**解决方案**:

**重新安装依赖**:
```bash
cd admin-backend/ui
rm -rf node_modules package-lock.json
npm install
```

**检查 Node.js 版本**:
```bash
node --version  # 应该 >= 18
npm --version
```

**手动启动前端**:
```bash
cd admin-backend/ui
npm run dev
```

---

### 5. 数据库容器启动失败

**错误信息**:
```
Error response from daemon: Conflict. The container name "/admin-backend-postgres" is already in use
```

**诊断步骤**:
```bash
# 查看所有容器（包括停止的）
docker ps -a | grep admin-backend-postgres

# 查看容器日志
docker logs admin-backend-postgres
```

**解决方案**:

**方案 1: 重启容器**
```bash
cd admin-backend
docker-compose restart postgres
```

**方案 2: 完全重建容器**
```bash
cd admin-backend
docker-compose down
docker-compose up -d postgres
```

**方案 3: 清理并重建（会删除数据）**
```bash
cd admin-backend
docker-compose down -v
docker-compose up -d postgres
```

---

### 6. API 请求失败

**错误信息**:
```
Failed to fetch
Network error
```

**诊断步骤**:
```bash
# 1. 检查后端是否运行
lsof -i :3000

# 2. 测试 API 端点
curl http://localhost:3000/health

# 3. 检查后端日志
tail -f /tmp/admin-backend.log
```

**解决方案**:

**后端未运行**:
```bash
cd admin-backend
cargo run
```

**CORS 错误**:
检查 `admin-backend/src/main.rs` 中的 CORS 配置

**数据库连接错误**:
参考"数据库连接错误"部分

---

### 7. 前端无法连接后端

**错误信息**:
```
ERR_CONNECTION_REFUSED
```

**诊断步骤**:
```bash
# 1. 检查后端是否运行
lsof -i :3000

# 2. 检查前端配置
cat admin-backend/ui/.env
```

**解决方案**:

**检查 API 基础 URL**:
```bash
# admin-backend/ui/.env
VITE_API_BASE_URL=http://localhost:3000
```

**重启前端**:
```bash
cd admin-backend/ui
npm run dev
```

---

## 完整重置流程

如果所有方法都失败，尝试完整重置：

```bash
# 1. 停止所有服务
pkill -f "admin-backend"
pkill -f "ironclaw.*run"
lsof -i :3000 | grep -v COMMAND | awk '{print $2}' | xargs kill -9
lsof -i :5174 | grep -v COMMAND | awk '{print $2}' | xargs kill -9

# 2. 停止并清理数据库
cd admin-backend
docker-compose down -v

# 3. 清理构建缓存
cargo clean
cd frontend
rm -rf node_modules package-lock.json

# 4. 重新安装依赖
cd ..
cargo build
cd frontend
npm install

# 5. 重新启动
cd ..
./scripts/start-admin.sh
```

---

## 日志位置

| 服务 | 日志位置 |
|------|---------|
| Admin Backend | `/tmp/admin-backend.log` |
| Admin Frontend | `/tmp/admin-frontend.log` |
| PostgreSQL | `docker logs admin-backend-postgres` |

---

## 常用调试命令

### 查看实时日志

```bash
# 后端日志
tail -f /tmp/admin-backend.log

# 前端日志
tail -f /tmp/admin-frontend.log

# 数据库日志
docker logs -f admin-backend-postgres

# 所有日志（多窗口）
# 终端 1
tail -f /tmp/admin-backend.log
# 终端 2
tail -f /tmp/admin-frontend.log
# 终端 3
docker logs -f admin-backend-postgres
```

### 测试数据库连接

```bash
# 方法 1: 使用 pg_isready
docker exec admin-backend-postgres pg_isready -U postgres

# 方法 2: 使用 psql
docker exec -it admin-backend-postgres psql -U postgres -d ironclaw -c "SELECT 1;"

# 方法 3: 使用 curl（通过后端）
curl http://localhost:3000/health
```

### 测试 API 端点

```bash
# 健康检查
curl http://localhost:3000/health

# 登录测试
curl -X POST http://localhost:3000/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"admin123"}'

# 获取 DLP 规则
curl http://localhost:3000/api/dlp-rules \
  -H "Authorization: Bearer YOUR_TOKEN"
```

---

## 获取帮助

如果以上方法都无法解决问题：

1. 收集所有日志：
   ```bash
   tail -100 /tmp/admin-backend.log > debug-backend.log
   tail -100 /tmp/admin-frontend.log > debug-frontend.log
   docker logs admin-backend-postgres > debug-db.log
   ```

2. 检查系统信息：
   ```bash
   # 操作系统
   uname -a
   
   # Rust 版本
   rustc --version
   cargo --version
   
   # Node.js 版本
   node --version
   npm --version
   
   # Docker 版本
   docker --version
   docker-compose --version
   ```

3. 提供错误信息和日志文件

---

## 预防措施

### 开发前检查清单

- [ ] Docker 已安装并运行
- [ ] Rust 已安装（最新稳定版）
- [ ] Node.js 已安装（>= 18）
- [ ] 端口 3000、5174、5432 未被占用
- [ ] `.env` 文件已配置
- [ ] 数据库容器已启动

### 每日开发流程

```bash
# 1. 启动数据库（如果未运行）
cd admin-backend
./scripts/start-db.sh

# 2. 启动后端
cargo run

# 3. 启动前端（新终端）
cd frontend
npm run dev
```

### 停止服务

```bash
# 按 Ctrl+C 停止后端和前端

# 停止数据库（可选，保留数据）
cd admin-backend
docker-compose down
```


## Desktop Client 启动问题

### 问题：连接被拒绝（Connection refused）

**症状**：
```
error sending request for url (http://localhost:38080/api/chat/threads): 
error trying to connect: tcp connect error: Connection refused (os error 61)
```

**原因**：
- IronClaw 服务器（端口 38080）未启动
- Desktop Client 现在使用外部 IronClaw 服务器，而不是内嵌服务器

**解决方案**：

1. 检查 IronClaw 服务器是否运行：
```bash
lsof -i :38080
curl http://localhost:38080/api/health
```

2. 如果没有运行，启动 IronClaw 服务器：
```bash
export GATEWAY_PORT=38080
export GATEWAY_HOST=127.0.0.1
export GATEWAY_ENABLED=true
cargo run --manifest-path ironclaw/Cargo.toml -- run --no-onboard
```

3. 或使用完整启动脚本（推荐）：
```bash
./scripts/start-all.sh
```

**架构说明**：
- 旧架构：Desktop Client 前端 → Tauri → 内嵌后端
- 新架构：Desktop Client 前端 → Tauri → 外部 IronClaw 服务器

**优点**：
- 避免复杂的内嵌启动逻辑
- 更容易调试和维护
- 与主项目保持一致的 API

### 问题：Tauri 客户端启动超时

**症状**：
```
⚠️  Tauri 启动超时，继续启动其他服务
⚠️  内嵌后端服务可能未启动，请检查 Tauri 日志
```

**原因**：
1. `desktop-client/src/embedded_server.rs` 使用了过时的 IronClaw API
2. 编译错误导致 Tauri 无法启动
3. 缺少 `dotenvy` 依赖

**解决方案**：

1. 添加缺失的依赖到 `desktop-client/Cargo.toml`：
```toml
dotenvy = "0.15"
```

2. 简化 `embedded_server.rs` 的实现，使用 CLI 方式启动 IronClaw：
```rust
// 使用 tokio::process::Command 启动 IronClaw CLI
let mut child = tokio::process::Command::new("cargo")
    .args(&["run", "--manifest-path", "../ironclaw/Cargo.toml", "--", "run", "--no-onboard"])
    .spawn()?;
```

3. 验证编译：
```bash
cd desktop-client
cargo build --no-default-features
```

4. 查看 Tauri 日志：
```bash
tail -f /tmp/tauri.log
```

**预防措施**：
- 遵循"外部库API使用验证策略"规则
- 在使用 IronClaw API 前，先查看主项目 `ironclaw/src/main.rs` 的实现
- 使用 `cargo build` 完整验证编译，不要只依赖 `getDiagnostics`
