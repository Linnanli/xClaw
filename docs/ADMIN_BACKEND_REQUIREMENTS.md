# 管理端后端功能需求文档

## 服务端信息

**地址**：`http://127.0.0.1:3000`  
**当前状态**：基础框架已实现，核心功能待开发

---

## 已实现的功能 ✅

### 1. 基础设施
- [x] Axum Web 框架集成
- [x] PostgreSQL 数据库连接池（deadpool-postgres）
- [x] 数据库迁移系统（refinery）
- [x] 环境变量配置（dotenvy）
- [x] 日志系统（tracing）
- [x] CORS 支持（tower-http）

### 2. 认证和授权
- [x] 用户注册 API (`POST /api/auth/register`)
- [x] 用户登录 API (`POST /api/auth/login`)
- [x] 令牌刷新 API (`POST /api/auth/refresh`)
- [x] JWT 令牌生成和验证
- [x] 密码哈希（Argon2）
- [x] 共享认证库（`ironclaw_auth`）

### 3. 用户管理
- [x] 获取用户信息 API (`GET /api/users/:id`)
- [x] 用户数据库表结构

### 4. 审计日志
- [x] 获取审计日志 API (`GET /api/audit-logs`)
- [x] 审计日志数据库表结构

### 5. DLP 规则管理
- [x] 获取 DLP 规则 API (`GET /api/dlp-rules`)
- [x] DLP 规则数据库表结构

### 6. 敏感操作管理
- [x] 获取敏感操作规则 API (`GET /api/sensitive-operations`)
- [x] 敏感操作规则数据库表结构

### 7. 策略查询（供桌面客户端使用）
- [x] 获取所有策略 API (`GET /api/policies`)
- [x] 获取 DLP 策略 API (`GET /api/policies/dlp`)
- [x] 获取敏感操作策略 API (`GET /api/policies/sensitive-ops`)
- [x] 获取策略版本 API (`GET /api/policies/version`)

### 8. 健康检查
- [x] 健康检查 API (`GET /health`)

---

## 待实现的核心功能 ❌

### 1. 用户管理（完整 CRUD）

#### 1.1 用户列表
- [ ] `GET /api/users` - 获取用户列表
  - 支持分页（page, page_size）
  - 支持搜索（username, email）
  - 支持排序（created_at, username）
  - 支持过滤（role, status）

#### 1.2 用户创建（管理员）
- [ ] `POST /api/users` - 创建用户（管理员权限）
  - 设置用户名、邮箱、密码
  - 分配角色（admin, user）
  - 设置初始状态（active, inactive）

#### 1.3 用户更新
- [ ] `PUT /api/users/:id` - 更新用户信息
  - 更新用户名、邮箱
  - 更新角色
  - 更新状态

#### 1.4 用户删除
- [ ] `DELETE /api/users/:id` - 删除用户（软删除）
  - 标记为已删除
  - 保留审计记录

#### 1.5 密码管理
- [ ] `POST /api/users/:id/change-password` - 修改密码
- [ ] `POST /api/users/:id/reset-password` - 重置密码（管理员）

---

### 2. 角色和权限管理（RBAC）

#### 2.1 角色管理
- [ ] `GET /api/roles` - 获取角色列表
- [ ] `POST /api/roles` - 创建角色
- [ ] `PUT /api/roles/:id` - 更新角色
- [ ] `DELETE /api/roles/:id` - 删除角色

#### 2.2 权限管理
- [ ] `GET /api/permissions` - 获取权限列表
- [ ] `POST /api/roles/:id/permissions` - 分配权限给角色
- [ ] `DELETE /api/roles/:id/permissions/:permission_id` - 移除角色权限

#### 2.3 用户角色分配
- [ ] `POST /api/users/:id/roles` - 分配角色给用户
- [ ] `DELETE /api/users/:id/roles/:role_id` - 移除用户角色

---

### 3. DLP 规则管理（完整 CRUD）

#### 3.1 DLP 规则列表
- [ ] `GET /api/dlp-rules` - 获取 DLP 规则列表（已有，需增强）
  - 支持分页
  - 支持搜索（pattern, severity）
  - 支持排序

#### 3.2 DLP 规则创建
- [ ] `POST /api/dlp-rules` - 创建 DLP 规则
  - 设置匹配模式（正则表达式）
  - 设置替换规则
  - 设置严重级别（low, medium, high, critical）
  - 设置动作（redact, block, warn）

#### 3.3 DLP 规则更新
- [ ] `PUT /api/dlp-rules/:id` - 更新 DLP 规则
  - 更新匹配模式
  - 更新替换规则
  - 更新严重级别

#### 3.4 DLP 规则删除
- [ ] `DELETE /api/dlp-rules/:id` - 删除 DLP 规则

#### 3.5 DLP 规则测试
- [ ] `POST /api/dlp-rules/test` - 测试 DLP 规则
  - 输入测试文本
  - 返回匹配结果和脱敏后的文本

#### 3.6 DLP 规则导入导出
- [ ] `POST /api/dlp-rules/import` - 导入 DLP 规则（JSON/CSV）
- [ ] `GET /api/dlp-rules/export` - 导出 DLP 规则（JSON/CSV）

---

### 4. 敏感操作规则管理（完整 CRUD）

#### 4.1 敏感操作规则列表
- [ ] `GET /api/sensitive-operations` - 获取敏感操作规则列表（已有，需增强）
  - 支持分页
  - 支持搜索
  - 支持排序

#### 4.2 敏感操作规则创建
- [ ] `POST /api/sensitive-operations` - 创建敏感操作规则
  - 设置操作类型（file_delete, system_command, network_access）
  - 设置是否需要批准
  - 设置批准者角色

#### 4.3 敏感操作规则更新
- [ ] `PUT /api/sensitive-operations/:id` - 更新敏感操作规则

#### 4.4 敏感操作规则删除
- [ ] `DELETE /api/sensitive-operations/:id` - 删除敏感操作规则

---

### 5. 审计日志管理（增强）

#### 5.1 审计日志列表
- [ ] `GET /api/audit-logs` - 获取审计日志列表（已有，需增强）
  - 支持分页
  - 支持时间范围过滤
  - 支持用户过滤
  - 支持操作类型过滤
  - 支持搜索

#### 5.2 审计日志详情
- [ ] `GET /api/audit-logs/:id` - 获取审计日志详情

#### 5.3 审计日志导出
- [ ] `GET /api/audit-logs/export` - 导出审计日志（CSV/JSON）

#### 5.4 审计日志统计
- [ ] `GET /api/audit-logs/stats` - 获取审计日志统计
  - 按用户统计
  - 按操作类型统计
  - 按时间统计

---

### 6. 策略版本管理

#### 6.1 策略版本列表
- [ ] `GET /api/policy-versions` - 获取策略版本列表
  - 显示版本号
  - 显示发布时间
  - 显示变更说明

#### 6.2 策略版本创建
- [ ] `POST /api/policy-versions` - 创建新的策略版本
  - 自动递增版本号
  - 记录变更说明
  - 生成 SM2 签名（如果启用国密）

#### 6.3 策略版本回滚
- [ ] `POST /api/policy-versions/:id/rollback` - 回滚到指定版本

#### 6.4 策略版本对比
- [ ] `GET /api/policy-versions/:id/diff` - 对比两个版本的差异

---

### 7. 桌面客户端管理

#### 7.1 客户端列表
- [ ] `GET /api/clients` - 获取桌面客户端列表
  - 显示客户端 ID
  - 显示用户信息
  - 显示最后活动时间
  - 显示在线状态

#### 7.2 客户端详情
- [ ] `GET /api/clients/:id` - 获取客户端详情
  - 显示客户端版本
  - 显示操作系统
  - 显示 IP 地址
  - 显示策略版本

#### 7.3 客户端策略推送
- [ ] `POST /api/clients/:id/push-policy` - 推送策略到指定客户端

#### 7.4 客户端强制下线
- [ ] `POST /api/clients/:id/force-offline` - 强制客户端下线

---

### 8. 技能（Skills）管理

#### 8.1 技能列表
- [ ] `GET /api/skills` - 获取技能列表
  - 支持分页
  - 支持搜索
  - 支持过滤（已审核、未审核）

#### 8.2 技能审核
- [ ] `POST /api/skills/:id/approve` - 审核通过技能
- [ ] `POST /api/skills/:id/reject` - 拒绝技能

#### 8.3 技能签名
- [ ] `POST /api/skills/:id/sign` - 使用 SM2 私钥签名技能

#### 8.4 技能发布
- [ ] `POST /api/skills/:id/publish` - 发布技能到企业商店

---

### 9. 扩展（Extensions）管理

#### 9.1 扩展列表
- [ ] `GET /api/extensions` - 获取扩展列表
  - 支持分页
  - 支持搜索
  - 支持过滤（已审核、未审核）

#### 9.2 扩展审核
- [ ] `POST /api/extensions/:id/approve` - 审核通过扩展
- [ ] `POST /api/extensions/:id/reject` - 拒绝扩展

#### 9.3 扩展签名
- [ ] `POST /api/extensions/:id/sign` - 使用 SM2 私钥签名扩展

#### 9.4 扩展发布
- [ ] `POST /api/extensions/:id/publish` - 发布扩展到企业商店

---

### 10. 系统配置管理

#### 10.1 系统配置列表
- [ ] `GET /api/system-config` - 获取系统配置列表
  - 会话超时时间
  - 令牌刷新间隔
  - DLP 启用状态
  - 国密算法启用状态

#### 10.2 系统配置更新
- [ ] `PUT /api/system-config/:key` - 更新系统配置

#### 10.3 系统配置导入导出
- [ ] `POST /api/system-config/import` - 导入系统配置
- [ ] `GET /api/system-config/export` - 导出系统配置

---

### 11. 统计和报表

#### 11.1 用户活动统计
- [ ] `GET /api/stats/user-activity` - 获取用户活动统计
  - 活跃用户数
  - 登录次数
  - 操作次数

#### 11.2 DLP 统计
- [ ] `GET /api/stats/dlp` - 获取 DLP 统计
  - 脱敏次数
  - 阻止次数
  - 按规则统计

#### 11.3 敏感操作统计
- [ ] `GET /api/stats/sensitive-ops` - 获取敏感操作统计
  - 批准次数
  - 拒绝次数
  - 按操作类型统计

#### 11.4 系统健康统计
- [ ] `GET /api/stats/system-health` - 获取系统健康统计
  - 在线客户端数
  - 数据库连接数
  - 内存使用情况

---

### 12. 通知和告警

#### 12.1 通知列表
- [ ] `GET /api/notifications` - 获取通知列表
  - 支持分页
  - 支持过滤（已读、未读）

#### 12.2 通知标记已读
- [ ] `POST /api/notifications/:id/read` - 标记通知为已读

#### 12.3 告警规则管理
- [ ] `GET /api/alert-rules` - 获取告警规则列表
- [ ] `POST /api/alert-rules` - 创建告警规则
- [ ] `PUT /api/alert-rules/:id` - 更新告警规则
- [ ] `DELETE /api/alert-rules/:id` - 删除告警规则

---

## 数据库表结构（需要补充）

### 已有表
- [x] `users` - 用户表
- [x] `audit_logs` - 审计日志表
- [x] `dlp_rules` - DLP 规则表
- [x] `sensitive_operation_rules` - 敏感操作规则表

### 待创建表
- [ ] `roles` - 角色表
- [ ] `permissions` - 权限表
- [ ] `role_permissions` - 角色权限关联表
- [ ] `user_roles` - 用户角色关联表
- [ ] `policy_versions` - 策略版本表
- [ ] `clients` - 桌面客户端表
- [ ] `skills` - 技能表
- [ ] `extensions` - 扩展表
- [ ] `system_config` - 系统配置表
- [ ] `notifications` - 通知表
- [ ] `alert_rules` - 告警规则表

---

## 优先级划分

### P0 - 核心功能（必须实现）
1. **用户管理完整 CRUD** - 管理员需要管理用户
2. **DLP 规则管理完整 CRUD** - 核心安全功能
3. **敏感操作规则管理完整 CRUD** - 核心安全功能
4. **审计日志增强** - 合规性要求
5. **角色和权限管理（RBAC）** - 访问控制

### P1 - 重要功能（建议实现）
6. **策略版本管理** - 策略变更追踪
7. **桌面客户端管理** - 监控和管理客户端
8. **统计和报表** - 运营数据分析
9. **系统配置管理** - 灵活配置

### P2 - 增强功能（可选实现）
10. **技能管理** - 企业技能商店
11. **扩展管理** - 企业扩展商店
12. **通知和告警** - 主动监控

---

## 技术栈

### 后端框架
- **Axum** - 高性能 Web 框架
- **Tokio** - 异步运行时
- **Tower** - 中间件支持

### 数据库
- **PostgreSQL** - 关系型数据库
- **Deadpool** - 连接池
- **Refinery** - 数据库迁移

### 认证和安全
- **ironclaw_auth** - 共享认证库（Argon2 + JWT）
- **JWT** - 令牌认证
- **Argon2** - 密码哈希

### 其他
- **Serde** - 序列化/反序列化
- **Tracing** - 日志和追踪
- **Dotenvy** - 环境变量

---

## 开发建议

### 第一阶段（1-2周）
1. 完善用户管理 CRUD
2. 实现角色和权限管理（RBAC）
3. 增强审计日志功能
4. 完善 DLP 规则管理 CRUD

### 第二阶段（1-2周）
5. 实现敏感操作规则管理 CRUD
6. 实现策略版本管理
7. 实现桌面客户端管理
8. 实现系统配置管理

### 第三阶段（1-2周）
9. 实现统计和报表
10. 实现技能和扩展管理
11. 实现通知和告警
12. 性能优化和测试

---

## 测试要求

### 单元测试
- 每个 API 端点都需要单元测试
- 测试覆盖率 > 80%

### 集成测试
- 测试完整的业务流程
- 测试数据库事务

### 属性测试
- 使用 `proptest` 进行属性测试
- 测试边界条件和异常情况

### 安全测试
- SQL 注入防护
- XSS 防护
- CSRF 防护
- 权限验证

---

## 文档要求

### API 文档
- 使用 OpenAPI/Swagger 生成 API 文档
- 每个端点都需要详细说明

### 部署文档
- Docker 部署指南
- 环境变量配置说明
- 数据库迁移指南

### 开发文档
- 代码结构说明
- 开发环境搭建
- 贡献指南

---

## 总结

管理端后端当前已完成基础框架和认证系统，但核心的 CRUD 功能、RBAC 权限管理、策略版本管理等关键功能尚未实现。

**预计总工作量**：4-6周（全职开发）

**建议优先级**：
1. 用户管理 CRUD + RBAC（1周）
2. DLP 和敏感操作规则管理（1周）
3. 审计日志增强 + 策略版本管理（1周）
4. 客户端管理 + 统计报表（1周）
5. 技能/扩展管理 + 通知告警（1-2周）

**服务端地址**：`http://127.0.0.1:3000`

您可以使用以下命令启动服务端：
```bash
cd admin-backend
cargo run
```

然后访问 `http://127.0.0.1:3000/health` 检查服务是否正常运行。
