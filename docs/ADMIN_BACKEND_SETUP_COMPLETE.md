# Admin Backend 环境搭建完成

**完成时间**：2026-03-19 17:40  
**状态**：✅ 全部完成，可以开始测试

---

## 已完成的工作 ✅

### 1. Docker PostgreSQL 环境

**配置文件**：
- `admin-backend/docker-compose.yml` - Docker Compose 配置
- `admin-backend/.env` - 环境变量配置

**PostgreSQL 信息**：
- 容器名称：admin-backend-postgres
- 镜像：postgres:15-alpine
- 主机：localhost
- 端口：5432
- 用户：postgres
- 密码：postgres
- 数据库：ironclaw

**状态**：✅ 运行中

### 2. Admin Backend 服务

**服务地址**：http://localhost:3000

**API 端点**：
- `GET /health` - 健康检查
- `POST /api/auth/register` - 用户注册
- `POST /api/auth/login` - 用户登录
- `POST /api/auth/refresh` - 刷新令牌
- `GET /api/users/{id}` - 获取用户信息
- `GET /api/audit-logs` - 获取审计日志
- `GET /api/dlp-rules` - 获取 DLP 规则
- `GET /api/sensitive-operations` - 获取敏感操作

**状态**：✅ 运行中

### 3. 测试用户

**账号信息**：
- 用户名：admin
- 密码：admin123
- 邮箱：admin@example.com
- 用户 ID：ddbf2b9c-0fb6-4dda-af00-2803c4df63d3

**状态**：✅ 已创建

### 4. 前端开发服务器

**服务地址**：http://localhost:5174

**功能**：
- 登录页面
- 仪表盘
- 主布局框架
- 路由系统
- 认证系统

**状态**：✅ 运行中

---

## 服务管理命令 🛠️

### Docker PostgreSQL

```bash
# 启动 PostgreSQL
cd admin-backend
docker-compose up -d

# 停止 PostgreSQL
docker-compose down

# 查看日志
docker-compose logs -f postgres

# 重启 PostgreSQL
docker-compose restart

# 连接数据库
docker-compose exec postgres psql -U postgres -d ironclaw

# 查看容器状态
docker-compose ps
```

### Admin Backend

```bash
# 启动服务
cd admin-backend
cargo run

# 或使用一键启动脚本
./scripts/start-with-docker.sh
```

### 前端开发服务器

```bash
# 启动前端
cd admin-backend/frontend
npm run dev

# 运行单元测试
npm test

# 运行 E2E 测试
npm run test:e2e
```

---

## 测试步骤 🧪

### 1. 测试登录功能

1. 打开浏览器访问：http://localhost:5174
2. 应该自动跳转到登录页面
3. 输入测试账号：
   - 用户名：admin
   - 密码：admin123
4. 点击登录按钮
5. 应该跳转到仪表盘页面

### 2. 测试仪表盘功能

登录成功后，应该看到：
- 4 个统计卡片（总用户数、在线用户、DLP 拦截、敏感操作）
- 2 个趋势图表（用户活动趋势、DLP 拦截趋势）
- 最近操作日志表格

### 3. 测试导航功能

- 点击侧边栏菜单切换页面
- 点击面包屑导航返回上级
- 点击用户菜单查看个人信息或退出登录

### 4. 测试 API

```bash
# 测试健康检查
curl http://localhost:3000/health

# 测试登录
curl -X POST http://localhost:3000/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"admin123"}'

# 测试获取用户信息（需要替换 USER_ID）
curl http://localhost:3000/api/users/ddbf2b9c-0fb6-4dda-af00-2803c4df63d3
```

---

## 数据库管理 💾

### 连接数据库

```bash
# 使用 Docker
docker-compose exec postgres psql -U postgres -d ironclaw

# 或使用本地 psql（如果已安装）
psql -h localhost -U postgres -d ironclaw
```

### 常用 SQL 命令

```sql
-- 查看所有表
\dt

-- 查看用户表
SELECT * FROM users;

-- 查看审计日志
SELECT * FROM audit_logs ORDER BY created_at DESC LIMIT 10;

-- 查看 DLP 规则
SELECT * FROM dlp_rules;

-- 删除测试用户（如需重新创建）
DELETE FROM users WHERE username = 'admin';
```

---

## 故障排查 🔧

### PostgreSQL 无法启动

```bash
# 检查 Docker 是否运行
docker info

# 检查容器状态
docker-compose ps

# 查看容器日志
docker-compose logs postgres

# 重启容器
docker-compose restart

# 完全重建容器
docker-compose down -v
docker-compose up -d
```

### Admin Backend 无法连接数据库

```bash
# 检查环境变量
cat admin-backend/.env

# 检查 PostgreSQL 是否就绪
docker-compose exec postgres pg_isready -U postgres

# 检查网络连接
telnet localhost 5432
```

### 前端无法连接后端

```bash
# 检查后端服务是否运行
curl http://localhost:3000/health

# 检查前端配置
cat admin-backend/frontend/src/api/client.ts

# 查看浏览器控制台错误
# 打开浏览器开发者工具 (F12) 查看 Network 和 Console
```

---

## 下一步工作 📋

### 立即可以做的

1. ✅ 测试登录功能
2. ✅ 测试仪表盘显示
3. ✅ 测试导航功能
4. ✅ 运行 E2E 测试

### 后续开发（P1）

5. 实现用户管理模块
6. 实现 DLP 规则管理
7. 实现审计日志
8. 实现角色和权限管理

### 增强功能（P2）

9. 实现敏感操作管理
10. 实现策略版本管理
11. 实现客户端管理
12. 实现技能和扩展管理
13. 实现系统配置
14. 实现统计报表

---

## 快速启动指南 🚀

### 方式 1：手动启动（推荐用于开发）

```bash
# 终端 1：启动 PostgreSQL
cd admin-backend
docker-compose up -d

# 终端 2：启动 Admin Backend
cd admin-backend
cargo run

# 终端 3：启动前端
cd admin-backend/frontend
npm run dev

# 浏览器：访问 http://localhost:5174
```

### 方式 2：使用启动脚本

```bash
cd admin-backend
./scripts/start-with-docker.sh
```

### 方式 3：分步启动

```bash
# 1. 启动 PostgreSQL
cd admin-backend
docker-compose up -d
sleep 5

# 2. 启动 Admin Backend
cargo run &

# 3. 等待服务启动
sleep 10

# 4. 创建测试用户（如果还没有）
./scripts/create_test_user.sh

# 5. 启动前端
cd frontend
npm run dev
```

---

## 环境信息 📊

### 系统环境

- 操作系统：macOS
- Docker 版本：29.2.1
- Node.js 版本：（需要检查）
- Rust 版本：（需要检查）

### 服务端口

| 服务 | 端口 | 状态 |
|------|------|------|
| PostgreSQL | 5432 | ✅ 运行中 |
| Admin Backend | 3000 | ✅ 运行中 |
| 前端开发服务器 | 5174 | ✅ 运行中 |

### 数据库信息

| 项目 | 值 |
|------|-----|
| 主机 | localhost |
| 端口 | 5432 |
| 用户 | postgres |
| 密码 | postgres |
| 数据库 | ironclaw |
| 容器名 | admin-backend-postgres |

---

## 总结 📝

✅ **已完成**：
1. Docker PostgreSQL 环境搭建
2. Admin Backend 服务启动
3. 测试用户创建
4. 前端开发服务器启动
5. 所有服务正常运行

🎉 **可以开始测试了！**

访问 http://localhost:5174 开始使用管理后台。

---

## 参考文档 📚

- `docs/ADMIN_FRONTEND_P0_COMPLETE.md` - 前端 P0 功能完成报告
- `docs/ADMIN_FRONTEND_STATUS_REPORT.md` - 前端状态报告
- `admin-backend/docker-compose.yml` - Docker 配置
- `admin-backend/.env` - 环境变量配置
- `admin-backend/scripts/start-with-docker.sh` - 启动脚本

