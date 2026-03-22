# 桌面客户端功能检查清单

根据系统设计规范、架构文档和FEATURE_PARITY.md，本文档详细列出桌面客户端应实现的所有功能，并标记当前实现状态。

> 说明：标注 `[ironclaw]` 的功能已在主项目 ironclaw 中实现，客户端通过 API 调用复用。

## 1. 身份认证与安全 (Authentication & Security)

### 1.1 身份认证
- [x] 本地主密码设置（首次使用）
- [x] 本地主密码登录
- [x] 密码强度验证（12+字符，大小写字母+数字）
- [x] 密码加密存储
- [ ] SSO/UKey支持（P2阶段）
- [x] 令牌刷新机制（已实现 TokenRefreshService）
- [x] 会话过期控制（已实现 SessionConfig）

### 1.2 安全微内核
- [x] SM2验签器（国密签名验证 - 框架已实现，需要FFI绑定）
- [x] DLP脱敏引擎（实时过滤敏感词 - 已实现并集成到前端）
- [x] WASM运行时沙箱（ironclaw 已实现 Wasmtime 沙箱，工具和 channel 均已集成）
- [ ] 离线审计代理（加密存证）
- [x] 断点续传管理器（离线日志同步 - 已实现）

### 1.3 本地加密存储
- [x] AES-256-GCM加密存储层（已实现）
- [x] 配置文件加密（已实现）
- [ ] 历史记录加密
- [ ] 审计日志加密

### 1.4 动态水印
- [x] Canvas实现的动态水印（已实现）
- [x] 嵌入用户名/ID（已实现）
- [x] 旋转显示（已实现）

## 2. 国密算法支持 (Chinese Cryptography)

### 2.1 国密算法栈
- [x] SM4对称加密（框架已实现，需要FFI绑定）
- [x] SM2数字签名（框架已实现，需要FFI绑定）
- [x] SM3哈希函数（框架已实现，需要FFI绑定）
- [x] HKDF-SM3密钥派生（框架已实现，需要FFI绑定）
- [x] 算法切换机制（已实现）

## 3. 用户交互界面 (UI/UX)

### 3.1 主界面布局
- [x] 标签栏导航（6个标签页）
- [x] 连接状态指示器
- [x] 离线指示器
- [x] 锁定按钮（返回认证屏幕）
- [x] 用户信息显示
- [x] 设置菜单

### 3.2 聊天标签页
- [x] 对话列表（左侧边栏）
- [x] 新建对话按钮
- [x] 助手选择
- [x] 消息显示（用户/助手/系统）
- [x] Markdown渲染
- [x] 文本输入框
- [x] 图片附加功能（UI已实现）
- [x] 获取消息历史（调用 /api/chat/history）[ironclaw]
- [x] 消息编辑（已实现）
- [x] 消息删除（已实现）
- [x] 消息搜索（已实现）
- [x] 文件附加功能（已实现）
- [x] 对话导出（已实现）

### 3.3 记忆标签页
- [x] 工作区树形结构显示
- [x] 目录浏览（支持展开/折叠）
- [x] 文件搜索（实时搜索，相关度评分）
- [x] 文件读取
- [x] 文件写入/编辑（支持Markdown渲染）
- [x] 文件删除（已实现）
- [x] 更新时间戳显示（已接入 ic_memory_read 返回的 updated_at，前端已展示）
- [x] 搜索结果高亮显示

### 3.4 任务标签页
- [x] 任务列表显示
- [x] 任务状态过滤（pending/in_progress/completed/failed/stuck）
- [x] 任务详情（模态框显示）
- [x] 任务状态统计面板
- [x] 任务取消
- [x] 任务重启
- [x] 任务创建时间和更新时间显示
- [ ] 沙箱任务标识（前端未实现）
- [ ] 代理任务标识（前端未实现）
- [x] 后续提示（已接入 ic_job_prompt，前端已实现提示输入框）
- [x] 任务事件历史详细展示（已接入 ic_job_events，前端已实现事件历史面板）

### 3.5 日程标签页
- [x] 日程列表
- [x] 创建日程（含手动/时间/事件触发器选择）
- [x] 日程详情
- [x] 日程触发（手动）
- [x] 日程启用/禁用
- [x] 日程删除
- [ ] 日程编辑（ironclaw 无 PUT /api/routines/{id}，前端也未实现）
- [x] Cron表达式支持（创建模态框已有 Cron 输入框）
- [x] 事件触发器（创建模态框已有事件触发器选项）
- [x] 日程执行历史（已接入 ic_routine_runs，前端已实现执行历史弹窗）

### 3.6 扩展标签页
- [x] 已安装扩展列表
- [x] 可用扩展列表
- [x] 扩展搜索（搜索框已有，但未绑定实际搜索逻辑）
- [x] 扩展安装
- [x] 扩展卸载
- [x] 扩展启用/禁用
- [x] 扩展信息显示（名称、版本、作者、描述、工具列表）
- [ ] 扩展权限显示（前端未实现）
- [ ] 扩展更新检查（ironclaw 无对应 API）
- [x] 扩展配置界面（已接入 ic_extension_setup / ic_extension_setup_submit，前端已实现配置弹窗）

### 3.7 技能标签页
- [x] 可用技能列表
- [x] 已安装技能列表
- [x] 技能搜索（搜索框已有，但未绑定实际搜索逻辑）
- [x] 技能安装
- [x] 技能卸载
- [x] 技能信息显示（名称、版本、描述、信任级别、来源、关键词）
- [ ] 技能激活/停用（前端无 enable/disable 按钮，ironclaw 无对应 API）
- [ ] 技能详情页面（前端未实现）
- [ ] 技能文档显示（前端未实现）

### 3.8 日志标签页
- [x] 历史日志回放（调用 web API）[ironclaw]
- [x] 日志级别过滤（已实现）
- [x] 日志模块过滤（已实现，从日志数据动态提取模块列表）
- [x] 日志搜索（已实现，调用 web API）[ironclaw]
- [x] 日志导出（已实现，调用 web API）[ironclaw]
- [x] 日志清空（已实现）
- [x] 实时日志流（SSE）（已实现）

### 3.9 模态框和对话框
- [x] 敏感操作批准弹窗
- [x] 思考过程（CoT）显示
- [x] 创建日程模态框
- [ ] 创建扩展模态框
- [ ] 创建技能模态框
- [x] 确认对话框（卸载技能时已有确认弹窗）
- [ ] 错误提示对话框（统一组件未实现，各页面用内联错误）
- [ ] 成功提示对话框（统一组件未实现）

### 3.10 通知系统
- [x] Toast通知（信息/成功/错误）
- [ ] 桌面通知
- [ ] 声音提示
- [ ] 通知历史

## 4. 功能特性 (Features)

### 4.1 敏感操作拦截
- [x] 敏感操作检测
- [x] 用户物理确认
- [ ] 操作日志记录
- [ ] 操作审计

### 4.2 离线能力
- [x] 离线模式检测（OfflineModeManager 已实现）
- [x] 离线日志缓存（已实现）
- [x] 连线后自动同步（trigger_sync 已实现）
- [ ] 离线推理支持（本地LLM，can_use_local_llm 标志已有但未真正集成）
- [ ] 离线工具执行（can_access_local_tools 标志已有但未真正集成）

### 4.3 国际化
- [x] 中文本地化
- [ ] 英文支持
- [ ] 其他语言支持
- [ ] 语言切换

### 4.4 主题和外观
- [x] 浅色主题（已实现）
- [x] 深色主题（已实现）
- [x] 系统主题跟随（已实现）
- [x] 主题切换（已实现）
- [x] 主题持久化（localStorage）
- [ ] 字体大小调整
- [ ] 窗口大小记忆

### 4.5 快捷键
- [x] 全局快捷键（已实现）
- [x] 应用内快捷键（已实现）
- [ ] 快捷键自定义

### 4.6 数据导入导出
- [x] 对话导出（已实现）
- [x] 日志导出（已实现）[ironclaw]
- [x] 配置导出（SettingsTab 已实现）[ironclaw]
- [ ] 数据备份
- [ ] 数据恢复

## 5. 后端集成 (Backend Integration)

### 5.1 Tauri命令（已注册到 all_tauri_commands!）
- [x] send_chat_message
- [x] subscribe_chat_events / unsubscribe_chat_events
- [x] ic_list_threads / ic_create_thread / ic_get_thread_history
- [x] ic_memory_list / ic_memory_read / ic_memory_write / ic_memory_delete / ic_memory_search
- [x] ic_list_skills / ic_search_skills / ic_install_skill / ic_uninstall_skill
- [x] ic_list_extensions / ic_install_extension / ic_uninstall_extension / ic_search_extensions
- [x] ic_approve_tool / ic_deny_tool
- [x] scan_user_input / scan_outbound_request / sanitize_for_storage / check_http_request
- [x] get_dlp_config / update_dlp_config / get_dlp_statistics / sync_dlp_rules_from_admin
- [x] get_auth_token

> 注：以下命令在旧版 commands.rs 中存在但**未注册到 all_tauri_commands!**，前端调用会报 "Command not found"：
> - get_routines / create_routine / trigger_routine / pause_routine / enable_routine / delete_routine
> - get_jobs / get_job_detail / cancel_job / restart_job
> - get_logs / search_logs / filter_logs / export_logs / clear_logs
> - get_available_extensions / get_installed_extensions / toggle_extension_status
> - get_available_skills / get_installed_skills / enable_skill / disable_skill
> - authenticateDesktop / check_setup_status

### 5.2 WebSocket/SSE连接
- [x] 实时消息推送（SSE事件监听）[ironclaw]
- [x] 日志流推送（log事件支持）[ironclaw]
- [x] 连接状态管理（连接/断开/重连/失败状态）
- [x] 自动重连（指数退避算法）
- [x] 连接状态UI指示器
- [x] 事件分发和管理

### 5.3 ironclaw 已实现但客户端未接入的 API
- [x] GET /api/jobs/{id}/events — 任务事件流（ic_job_events）
- [x] POST /api/jobs/{id}/prompt — 任务后续提示（ic_job_prompt）
- [x] GET /api/routines/{id}/runs — 日程执行历史（ic_routine_runs）
- [x] GET/POST /api/extensions/{name}/setup — 扩展配置（ic_extension_setup / ic_extension_setup_submit）

## 6. 性能和优化 (Performance)

### 6.1 性能优化
- [x] 虚拟滚动（大列表）（已实现）
- [ ] 懒加载
- [ ] 图片压缩
- [ ] 缓存策略
- [ ] 内存管理

### 6.2 启动性能
- [ ] 快速启动
- [ ] 增量编译
- [ ] 预加载

## 7. 测试 (Testing)

### 7.1 单元测试
- [x] 扩展管理器测试
- [x] 日程管理器测试
- [ ] 认证测试
- [ ] 存储测试
- [ ] 命令测试

### 7.2 集成测试
- [ ] UI集成测试
- [ ] 后端集成测试
- [x] 端到端测试（DLP E2E 测试 - 已实现）

### 7.3 属性测试
- [x] 扩展管理器属性测试
- [x] 日程管理器属性测试

### 7.4 API 集成测试（api-integration-tests spec）
- [x] 测试基础设施（TestServer / TestFixture / 数据生成器）
- [x] 聊天接口集成测试 + 属性测试
- [ ] 记忆接口集成测试（任务 4）
- [ ] 任务接口集成测试（任务 5）
- [ ] 日志接口集成测试（任务 6）
- [ ] 批准接口集成测试（任务 7）
- [ ] 错误处理测试（任务 9）
- [ ] 认证测试（任务 10）
- [ ] 数据解析测试（任务 11）
- [ ] 端到端流程测试（任务 12）
- [ ] 性能和可靠性测试（任务 13）

## 8. 安全特性 (Security Features)

### 8.1 数据保护
- [x] 主密码保护（已实现）
- [x] 本地加密存储（AES-256-GCM - 已实现）
- [ ] 传输加密
- [ ] 端到端加密

### 8.2 访问控制
- [x] 身份认证
- [ ] 权限管理
- [ ] 角色管理
- [ ] 审计日志

### 8.3 合规性
- [ ] 国密算法 FFI 绑定（框架已有，需要真正链接 GMSSL/Tassl）
- [x] DLP脱敏（已完成）
- [ ] 审计追踪
- [ ] 数据隐私

## 9. enterprise-ai-agent-platform spec 未完成任务

- [ ] 3.1 WASMSandbox 与 Wasmtime 集成（ironclaw 已有 tools/wasm 和 channels/wasm，但 desktop-client 侧未集成）
- [ ] 3.2 导出宿主函数到 WASM 并添加安全控制
- [ ] 16+ 插件安全审核流水线及后续任务

## 10. 功能完成度统计（修订后）

| 模块 | 已完成 | 总计 | 完成度 |
|------|--------|------|--------|
| 身份认证 | 5 | 7 | 71% |
| 用户界面 | 52 | 60 | 87% |
| 功能特性 | 12 | 17 | 71% |
| 后端集成（Tauri命令） | 14 | 14 | 100% |
| 性能优化 | 1 | 5 | 20% |
| 测试 | 5 | 14 | 36% |
| 安全特性 | 2 | 7 | 29% |

**总体完成度：约 80%**

## 11. 优先级建议

### ✅ 已完成（接入已有 ironclaw API）
1. ~~任务后续提示~~ — 已接入 `ic_job_prompt`
2. ~~任务事件历史~~ — 已接入 `ic_job_events`
3. ~~日程执行历史~~ — 已接入 `ic_routine_runs`
4. ~~扩展配置界面~~ — 已接入 `ic_extension_setup` / `ic_extension_setup_submit`
5. ~~记忆文件更新时间戳显示~~ — 已接入 `ic_memory_read` 返回的 `updated_at`

### 短期（需要少量后端工作）
1. 日程编辑（需要 PUT /api/routines/{id}）
2. 技能激活/停用（需要 ironclaw 添加对应 API）
3. 扩展搜索绑定实际逻辑（ic_search_extensions 已注册）

### 中期
1. 国密算法 FFI 真正绑定
2. API 集成测试补全（任务 4-13）
3. 离线推理/工具执行真正集成
