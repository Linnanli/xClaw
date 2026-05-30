# 设计图 vs 代码实现差距分析

> 设计图文件：`desktop-client/ui/design/client-design.pen`
> 分析日期：2026-03-24

## ✅ 已实现功能

| # | 设计图页面 | 节点 ID | 代码文件 | 状态 |
|---|-----------|---------|---------|------|
| 1 | X-Claw Chat V2 | `0h3W2` | `ChatTabTauri.tsx` | ✅ 完成 |
| 2 | Sidebar 导航 | 各页面共用 | `AppSidebar.tsx` | ✅ 完成 |
| 3 | Header 顶部栏 | 各页面共用 | `AppHeader.tsx` | ✅ 完成 |
| 4 | Settings Modal | `WAp70` / `Jo8Lp` | `SettingsModal.tsx` | ✅ 完成（6个子页面） |
| 5 | Jobs Popup | `9sHdc` | `JobsPanel.tsx` | ✅ 完成 |
| 6 | Notifications | `XIev7` | `NotificationsPanel.tsx` | ✅ 完成 |
| 7 | Routines Popup | `OTmdL` | `RoutinesTab.tsx` | ✅ 完成 |
| 8 | Login（本地密码版） | `wXR8X` | `PasswordLogin.tsx` | ✅ 部分 |
| 9 | DLP Warning | `IRka8` | `DlpWarningBanner/DlpBlockedDialog` | ✅ 完成 |
| 10 | Aurora 极光动画 | `IRka8` | `aurora.css` | ✅ 完成 |
| 11 | Dark Theme | `OfA0N` | `ThemeContext + ThemeToggle` | ✅ 完成 |
| 12 | Thinking Process | `Clux6` | `ThinkingProcess.tsx` | ✅ 完成 |
| 13 | Theme Toggle | `nDb03` | `ThemeToggle.tsx` | ✅ 完成 |

## ❌ 未实现功能

### P0 - 核心功能

| # | 设计图页面 | 节点 ID | 描述 | 优先级 |
|---|-----------|---------|------|--------|
| 1 | Model Select 下拉 | `Jd2oJ` / `WJK0X` | 模型选择下拉菜单（240px，含自定义模型入口） | P0 |
| 2 | Custom Model 弹窗 | `8N9eQ` / `YPJKU` | 自定义模型管理（600x652，添加/列表/测试连接） | P0 |
| 3 | Edit Model 弹窗 | `kwwDg` / `yFbWq` | 编辑已有模型（同结构，选中项绿色边框） | P0 |
| 4 | Approval 审批页 | `XvUIc` | 聊天中嵌入审批卡片（批准/拒绝操作） | P0 |
| 5 | Lock Screen 锁屏 | `jEF13` | 深色全屏锁屏（密码输入 + SSO 重认证） | P0 |
| 6 | Team Switch 团队切换 | `Lv6S9` / `SEVVI` | 侧边栏底部工作区切换下拉（220px） | P0 |

### P1 - 重要功能

| # | 设计图页面 | 节点 ID | 描述 | 优先级 |
|---|-----------|---------|------|--------|
| 7 | Knowledge Base 知识库 | `Zp4Gq` | 独立知识库页面（文件管理 + 搜索） | P1 |
| 8 | 通知审批操作 | `XIev7`-`SVHaw` | 审批类通知含"去审批"按钮 | P1 |
| 9 | 任务取消按钮 | `9sHdc`-`Zn6g5` | 运行中任务的"取消"操作 | P1 |
| 10 | Confirm Dialog | `2HGWJ` / `qS6VL` | 通用确认弹窗（图标 + 标题 + 危险操作按钮） | P1 |

### P2 - 细节完善

| # | 设计图页面 | 节点 ID | 描述 | 优先级 |
|---|-----------|---------|------|--------|
| 11 | Login 品牌区 | `wXR8X`-`02F8C` | 左侧绿色品牌区 + SSO 登录选项 | P2 |
| 12 | 设置-用户信息 | `WAp70`-`WfC4b` | 头像上传 + 用户名编辑 | P2 |
| 13 | Thinking Complete | `uQBD1` | 思考完成折叠/展开状态样式 | P2 |

## 实施计划

### Phase 1：模型管理（当前）
- [x] 差距分析文档
- [ ] Admin Backend: 模型配置 API（CRUD + 下发）
- [ ] Desktop Client: Tauri IPC 模型配置命令
- [ ] 前端: ModelSelector 下拉菜单（匹配设计图 `WJK0X`）
- [ ] 前端: CustomModelModal 弹窗（匹配设计图 `YPJKU`）
- [ ] 测试: 单元 + 契约 + 失败路径 + 安全审计

### Phase 2：审批 + 锁屏
- [ ] Approval 审批 UI 组件
- [ ] Lock Screen 锁屏页面
- [ ] Confirm Dialog 通用确认弹窗

### Phase 3：团队 + 知识库
- [ ] Team Switch 团队切换
- [ ] Knowledge Base 知识库页面

### Phase 4：细节完善
- [ ] Login 品牌区 + SSO
- [ ] 通知审批操作
- [ ] 任务取消按钮
- [ ] 用户信息编辑
