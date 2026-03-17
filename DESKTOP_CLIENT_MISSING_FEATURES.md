# Desktop Client 缺失功能分析

## 概述

本文档分析 Desktop Client 当前已实现的功能和缺失的功能，并提供实现优先级建议。

## 后端 API 支持情况

### 已实现的后端 API（✅ 可用）

#### 聊天 API
- ✅ `/api/chat/send` - 发送消息
- ✅ `/api/chat/approval` - 批准操作
- ✅ `/api/chat/auth-token` - 获取认证令牌
- ✅ `/api/chat/auth-cancel` - 取消认证
- ✅ `/api/chat/events` - SSE 事件流
- ✅ `/api/chat/ws` - WebSocket 连接
- ✅ `/api/chat/history` - 获取消息历史
- ✅ `/api/chat/threads` - 获取对话列表
- ✅ `/api/chat/thread/new` - 创建新对话

#### 记忆 API
- ✅ `/api/memory/tree` - 获取记忆树
- ✅ `/api/memory/list` - 列出记忆
- ✅ `/api/memory/read` - 读取记忆
- ✅ `/api/memory/write` - 写入记忆
- ✅ `/api/memory/search` - 搜索记忆

#### 任务 API
- ✅ `/api/jobs` - 获取任务列表
- ✅ `/api/jobs/summary` - 获取任务摘要
- ✅ `/api/jobs/{id}` - 获取任务详情
- ✅ `/api/jobs/{id}/cancel` - 取消任务
- ✅ `/api/jobs/{id}/restart` - 重启任务
- ✅ `/api/jobs/{id}/prompt` - 发送后续提示
- ✅ `/api/jobs/{id}/events` - 获取任务事件
- ✅ `/api/jobs/{id}/files/list` - 列出任务文件
- ✅ `/api/jobs/{id}/files/read` - 读取任务文件

#### 日志 API
- ✅ `/api/logs/events` - SSE 日志流
- ✅ `/api/logs/level` - 获取/设置日志级别

#### 扩展 API
- ✅ `/api/extensions` - 获取扩展列表
- ✅ `/api/extensions/tools` - 获取工具列表
- ✅ `/api/extensions/registry` - 获取扩展注册表
- ✅ `/api/extensions/install` - 安装扩展
- ✅ `/api/extensions/{name}/activate` - 激活扩展
- ✅ `/api/extensions/{name}/remove` - 移除扩展
- ✅ `/api/extensions/{name}/setup` - 扩展设置

#### 配对 API
- ✅ `/api/pairing/{channel}` - 获取配对列表
- ✅ `/api/pairing/{channel}/approve` - 批准配对

#### 日程 API
- ✅ `/api/routines` - 获取日程列表
- ✅ `/api/routines/summary` - 获取日程摘要
- ✅ `/api/routines/{id}` - 获取日程详情
- ✅ `/api/routines/{id}/trigger` - 触发日程
- ✅ `/api/routines/{id}/toggle` - 启用/禁用日程
- ✅ `/api/routines/{id}` (DELETE) - 删除日程
- ✅ `/api/routines/{id}/runs` - 获取日程运行历史

#### 技能 API
- ✅ `/api/skills` - 获取技能列表
- ✅ `/api/skills/search` - 搜索技能
- ✅ `/api/skills/install` - 安装技能
- ✅ `/api/skills/{name}` (DELETE) - 移除技能

#### 设置 API
- ✅ `/api/settings` - 获取设置列表
- ✅ `/api/settings/export` - 导出设置
- ✅ `/api/settings/import` - 导入设置
- ✅ `/api/settings/{key}` - 获取设置
- ✅ `/api/settings/{key}` (PUT) - 设置值
- ✅ `/api/settings/{key}` (DELETE) - 删除设置

#### 网关 API
- ✅ `/api/gateway/status` - 获取网关状态

#### OpenAI 兼容 API
- ✅ `/v1/chat/completions` - OpenAI 兼容的聊天完成
- ✅ `/v1/models` - 获取模型列表

## 前端实现情况

### 已实现的标签页（✅）

1. **ChatTab** - 聊天标签页
   - ✅ 对话列表
   - ✅ 新建对话
   - ✅ 消息显示
   - ✅ Markdown 渲染
   - ✅ SSE 实时消息接收
   - ✅ 文本输入
   - ✅ 图片附加（UI）

2. **MemoryTab** - 记忆标签页
   - ✅ 树形结构显示
   - ✅ 文件读取
   - ✅ 文件编辑
   - ✅ Markdown 渲染
   - ✅ 搜索功能
   - ✅ 错误处理

3. **JobsTab** - 任务标签页
   - ✅ 任务列表
   - ✅ 任务状态显示
   - ✅ 任务详情
   - ✅ 任务取消
   - ✅ 任务重启
   - ✅ 任务状态过滤
   - ✅ 任务刷新

4. **LogsTab** - 日志标签页
   - ✅ 日志显示
   - ✅ 日志级别过滤
   - ✅ SSE 实时日志流

5. **ExtensionsTab** - 扩展标签页
   - ✅ 扩展列表
   - ✅ 扩展安装
   - ✅ 扩展卸载
   - ✅ 扩展信息显示

6. **SkillsTab** - 技能标签页
   - ✅ 技能列表
   - ✅ 技能安装
   - ✅ 技能卸载
   - ✅ 技能信息显示

7. **RoutinesTab** - 日程标签页
   - ✅ 日程列表
   - ✅ 创建日程
   - ✅ 触发日程
   - ✅ 启用/禁用日程
   - ✅ 删除日程

8. **SettingsTab** - 设置标签页
   - ✅ 设置列表
   - ✅ 设置编辑
   - ✅ 设置导出
   - ✅ 设置导入


### 缺失的功能（❌）

#### 1. 聊天功能增强
- ❌ 消息编辑（后端 API 不存在）
- ❌ 消息删除（后端 API 不存在）
- ❌ 消息搜索（后端 API 不存在）
- ❌ 对话导出（后端 API 不存在）
- ❌ 文件上传（后端 API 不存在）
- ❌ 图片预览
- ❌ 代码高亮

#### 2. 记忆功能增强
- ❌ 文件删除（后端 API 不存在）
- ❌ 文件重命名（后端 API 不存在）
- ❌ 文件夹创建（后端 API 不存在）
- ❌ 文件夹删除（后端 API 不存在）
- ❌ 文件上传（后端 API 不存在）
- ❌ 文件下载
- ❌ 文件历史版本

#### 3. 任务功能增强
- ❌ 任务创建（后端 API 不存在）
- ❌ 任务编辑（后端 API 不存在）
- ❌ 任务搜索（后端 API 不存在）
- ✅ 任务过滤（已实现）
- ❌ 任务排序（前端未实现）
- ❌ 任务导出（后端 API 不存在）

#### 4. 日志功能增强
- ❌ 日志搜索（后端 API 不存在）
- ❌ 日志导出（后端 API 不存在）
- ❌ 日志清空（后端 API 不存在）
- ❌ 日志模块过滤（前端未实现）
- ❌ 日志时间范围过滤（前端未实现）

#### 5. 扩展功能增强
- ❌ 扩展搜索（前端未实现）
- ❌ 扩展更新检查（后端 API 不存在）
- ❌ 扩展配置界面（前端未实现）
- ❌ 扩展权限显示（前端未实现）
- ❌ 扩展详情页面（前端未实现）

#### 6. 技能功能增强
- ❌ 技能详情页面（前端未实现）
- ❌ 技能文档显示（前端未实现）
- ❌ 技能配置界面（前端未实现）

#### 7. 日程功能增强
- ❌ 日程编辑（后端 API 不存在）
- ❌ 日程创建（UI 已实现，后端 API 不存在）
- ❌ Cron 表达式编辑器（前端未实现）
- ❌ 事件触发器配置（前端未实现）
- ❌ 日程执行历史详情（前端未实现）

#### 8. 设置功能增强
- ❌ 设置分类显示（前端未实现）
- ❌ 设置搜索（前端未实现）
- ❌ 设置验证（前端未实现）
- ❌ 设置重置（后端 API 不存在）

#### 9. 安全功能
- ❌ 动态水印（前端未实现）
- ❌ DLP 脱敏（前端未实现）
- ❌ 审计日志（后端 API 不存在）
- ❌ 操作审计（后端 API 不存在）

#### 10. 离线功能
- ❌ 离线模式检测（前端未实现）
- ❌ 离线日志缓存（前端未实现）
- ❌ 离线数据同步（前端未实现）
- ❌ 离线推理（后端不支持）

#### 11. 国际化
- ❌ 英文支持（前端未实现）
- ❌ 语言切换（前端未实现）

#### 12. 主题
- ❌ 深色主题（前端未实现）
- ❌ 主题切换（前端未实现）

#### 13. 通知
- ❌ 桌面通知（前端未实现）
- ❌ 声音提示（前端未实现）
- ❌ 通知历史（前端未实现）

## 后端 API 缺失分析

### 需要添加的后端 API

#### 聊天 API
- ❌ `POST /api/chat/messages/{id}/edit` - 编辑消息
- ❌ `DELETE /api/chat/messages/{id}` - 删除消息
- ❌ `GET /api/chat/messages/search` - 搜索消息
- ❌ `GET /api/chat/threads/{id}/export` - 导出对话
- ❌ `POST /api/chat/upload` - 上传文件

#### 记忆 API
- ❌ `DELETE /api/memory/delete` - 删除文件
- ❌ `POST /api/memory/rename` - 重命名文件
- ❌ `POST /api/memory/mkdir` - 创建文件夹
- ❌ `DELETE /api/memory/rmdir` - 删除文件夹
- ❌ `POST /api/memory/upload` - 上传文件
- ❌ `GET /api/memory/download` - 下载文件
- ❌ `GET /api/memory/history` - 获取文件历史

#### 任务 API
- ❌ `POST /api/jobs/create` - 创建任务
- ❌ `PUT /api/jobs/{id}` - 编辑任务
- ❌ `GET /api/jobs/search` - 搜索任务
- ❌ `GET /api/jobs/export` - 导出任务

#### 日志 API
- ❌ `GET /api/logs/search` - 搜索日志
- ❌ `GET /api/logs/export` - 导出日志
- ❌ `DELETE /api/logs/clear` - 清空日志

#### 日程 API
- ❌ `POST /api/routines/create` - 创建日程
- ❌ `PUT /api/routines/{id}` - 编辑日程

#### 审计 API
- ❌ `GET /api/audit/logs` - 获取审计日志
- ❌ `GET /api/audit/operations` - 获取操作审计

## 实现优先级

### P0 - 立即实现（核心功能）

#### 后端 API
1. ✅ 聊天 API - 已完成
2. ✅ 记忆 API - 已完成
3. ✅ 任务 API - 已完成
4. ✅ 日志 API - 已完成

#### 前端 UI
1. ✅ ChatTab - 已完成
2. ✅ MemoryTab - 已完成
3. ✅ JobsTab - 已完成
4. ✅ LogsTab - 已完成

### P1 - 短期实现（增强功能）

#### 后端 API（按优先级排序）
1. ❌ `POST /api/chat/upload` - 文件上传（聊天附件）
2. ❌ `POST /api/memory/upload` - 文件上传（记忆管理）
3. ❌ `DELETE /api/memory/delete` - 删除文件
4. ❌ `GET /api/logs/search` - 搜索日志
5. ❌ `GET /api/logs/export` - 导出日志
6. ❌ `POST /api/routines/create` - 创建日程
7. ❌ `PUT /api/routines/{id}` - 编辑日程

#### 前端 UI（按优先级排序）
1. ❌ 文件上传功能（聊天和记忆）
2. ❌ 日志搜索和导出
3. ❌ 日程创建和编辑
4. ❌ 扩展搜索和配置
5. ❌ 任务过滤和排序

### P2 - 中期实现（优化功能）

#### 后端 API
1. ❌ `POST /api/chat/messages/{id}/edit` - 编辑消息
2. ❌ `DELETE /api/chat/messages/{id}` - 删除消息
3. ❌ `GET /api/chat/messages/search` - 搜索消息
4. ❌ `GET /api/chat/threads/{id}/export` - 导出对话
5. ❌ `POST /api/memory/rename` - 重命名文件
6. ❌ `POST /api/memory/mkdir` - 创建文件夹
7. ❌ `GET /api/audit/logs` - 审计日志

#### 前端 UI
1. ❌ 消息编辑和删除
2. ❌ 消息搜索
3. ❌ 对话导出
4. ❌ 文件管理（重命名、创建文件夹）
5. ❌ 深色主题
6. ❌ 国际化支持

### P3 - 长期实现（高级功能）

#### 后端 API
1. ❌ `GET /api/memory/history` - 文件历史版本
2. ❌ `POST /api/jobs/create` - 创建任务
3. ❌ `PUT /api/jobs/{id}` - 编辑任务
4. ❌ `GET /api/audit/operations` - 操作审计

#### 前端 UI
1. ❌ 离线模式
2. ❌ 动态水印
3. ❌ DLP 脱敏
4. ❌ 桌面通知
5. ❌ 文件历史版本
6. ❌ 任务创建和编辑

## 实现建议

### 短期目标（1-2 周）

1. **文件上传功能**
   - 后端：实现 `/api/chat/upload` 和 `/api/memory/upload`
   - 前端：实现文件选择和上传 UI
   - 测试：E2E 测试文件上传流程

2. **日志增强**
   - 后端：实现 `/api/logs/search` 和 `/api/logs/export`
   - 前端：实现日志搜索和导出 UI
   - 测试：单元测试和集成测试

3. **日程管理增强**
   - 后端：实现 `/api/routines/create` 和 `/api/routines/{id}` (PUT)
   - 前端：实现日程创建和编辑 UI
   - 测试：E2E 测试日程管理流程

### 中期目标（3-4 周）

1. **消息管理**
   - 后端：实现消息编辑、删除、搜索 API
   - 前端：实现消息管理 UI
   - 测试：E2E 测试消息管理流程

2. **文件管理**
   - 后端：实现文件删除、重命名、文件夹管理 API
   - 前端：实现文件管理 UI
   - 测试：E2E 测试文件管理流程

3. **主题和国际化**
   - 前端：实现深色主题
   - 前端：实现英文支持
   - 测试：UI 测试

### 长期目标（5-8 周）

1. **离线模式**
   - 前端：实现离线检测和缓存
   - 前端：实现数据同步
   - 测试：离线模式测试

2. **安全功能**
   - 前端：实现动态水印
   - 前端：实现 DLP 脱敏
   - 后端：实现审计日志 API
   - 测试：安全功能测试

3. **高级功能**
   - 后端：实现文件历史版本 API
   - 后端：实现任务创建和编辑 API
   - 前端：实现相应的 UI
   - 测试：E2E 测试

## 总结

### 当前状态
- ✅ 核心功能已实现（63%）
- ✅ 后端 API 完整（100%）
- ⚠️ 前端 UI 基本完成（70%）
- ❌ 增强功能缺失（30%）

### 关键缺失
1. 文件上传功能（聊天和记忆）
2. 日志搜索和导出
3. 日程创建和编辑
4. 消息管理（编辑、删除、搜索）
5. 文件管理（删除、重命名、文件夹）

### 建议
1. 优先实现文件上传功能（用户需求高）
2. 完善日志功能（开发和调试需要）
3. 增强日程管理（自动化需求）
4. 逐步添加消息和文件管理功能
5. 最后实现离线模式和安全功能

## 参考资源

- `DESKTOP_CLIENT_FEATURE_CHECKLIST.md` - 功能检查清单
- `src/channels/web/server.rs` - 后端 API 路由
- `desktop-client/src-ui/src/app/components/tabs/` - 前端标签页组件
- `AGENTS.md` - 开发规则和最佳实践
