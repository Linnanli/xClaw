# 功能验证操作手册

本手册覆盖以下 4 项改动的端到端验证：
1. 对话流工具授权按钮
2. 日志 Tab 实时数据
3. 定时任务执行历史
4. 扩展 Tab 刷新按钮

---

## P2 验证补充

本轮新增的 P2 能力还需要补充以下验证：
1. Plan Mode 结构化计划输出与审批前等待状态
2. Session Fork 分支创建与独立演进
3. Sub-Agent Explore / Verify 角色与安全边界

### Rust 侧自动化验证

```bash
cargo test -p ironclaw --test parity_gate_p2
cargo test -p ironclaw --test p2_e2e_tests
cargo test -p ironclaw --test p2_security_audit_tests
```

### 桌面端前端验证重点

1. 发送触发规划的请求后，应看到 Plan 卡片而不是普通纯文本。
2. 从历史消息重载线程时，Plan / Fork / Sub-Agent 工具卡片应能被恢复。
3. Verify 子代理场景不得暴露原始敏感输入到 UI 错误提示。
4. Plan 未批准前，不得实际执行中高风险写操作。

---

## 第一步：启动环境

### 1.1 启动 Docker

打开 Docker Desktop，等待菜单栏图标变为正常状态，然后确认：

```bash
docker ps
```

### 1.2 一键启动所有服务

```bash
./scripts/start-all.sh
```

等待终端输出以下内容后继续：

```
✅ Admin Backend 后端已启动 (http://localhost:3000)
✅ Admin Backend 前端已启动 (http://localhost:5174)
✅ Desktop Client 已启动
```

### 1.3 确认服务正常

| 服务 | 地址 | 预期结果 |
|------|------|---------|
| 管理后台 | http://localhost:5174 | 显示登录页 |
| 后端 API | http://localhost:3000/health | 返回 `{"status":"ok"}` |
| 客户端 | Tauri 窗口自动打开 | 显示密码登录页 |

---

## 第二步：登录管理后台

1. 打开 http://localhost:5174
2. 用户名：`admin`，密码：`admin123`
3. 登录成功后进入仪表盘

---

## 第三步：验证日志 Tab

> 目标：确认日志 Tab 能显示真实的引擎运行日志，搜索/过滤/导出/清空功能正常。

### 3.1 打开日志 Tab

在客户端 Tauri 窗口中，点击左侧导航栏的「日志」图标。

**预期**：日志列表显示引擎启动过程中产生的日志条目（不再是空列表）。典型条目包括：
- `IronClaw embedded engine starting...`
- `Configuration loaded`
- `DLP rules synced from admin backend`

如果列表为空，等待 5 秒后点击页面空白处触发刷新，或重新点击「日志」Tab。

### 3.2 测试搜索

在搜索框输入 `engine`，列表应只显示包含 "engine" 的日志条目。

清空搜索框，列表恢复全部显示。

### 3.3 测试级别过滤

下拉选择「警告」，列表只显示 warn 级别的日志。

下拉选择「所有级别」，恢复全部显示。

### 3.4 测试导出

点击右上角「导出」按钮，浏览器应下载一个 `logs-{时间戳}.json` 文件。

打开文件，确认内容是 JSON 数组，每条记录包含 `timestamp`、`level`、`module`、`message` 字段。

### 3.5 测试清空

点击右上角「清空」按钮，弹出确认对话框，点击确认。

**预期**：日志列表变为空。

在客户端发送一条聊天消息（任意内容），然后重新点击「日志」Tab，应该能看到新产生的日志（旧日志不再出现）。

---

## 第四步：验证定时任务执行历史

> 目标：确认定时任务卡片底部显示真实的执行时间和次数，而不是硬编码的 `-` 和 `0`。

### 4.1 打开定时任务面板

点击客户端顶部工具栏的「定时任务」按钮（时钟图标），或通过菜单打开。

### 4.2 创建一个测试任务（如果没有现有任务）

点击右上角「新建定时任务」：
- 名称：`测试任务`
- 描述：`用于验证执行历史`
- 触发方式：`手动`

点击「创建」。

### 4.3 手动触发任务

在任务卡片右侧点击「运行」按钮，触发一次执行。

等待 2-3 秒后，关闭面板再重新打开。

**预期**：任务卡片底部元信息行显示：
- 「上次执行：XX月XX日 HH:MM」（真实时间，不是 `-`）
- 「已执行 1 次」（不是 `0`）

再次点击「运行」，重新打开面板，次数变为 `2`。

---

## 第五步：验证扩展 Tab 刷新按钮

> 目标：确认「刷新扩展」按钮能触发列表重新加载。

### 5.1 打开扩展 Tab

点击左侧导航栏的「扩展」图标。

### 5.2 点击刷新按钮

右上角按钮现在显示「刷新扩展」（之前是「添加扩展」且点击无效）。

点击该按钮。

**预期**：列表短暂显示「加载中...」，然后重新显示扩展列表。

### 5.3 验证按钮有效性（开发者工具）

打开 Tauri 开发者工具（`Cmd+Option+I`），切换到 Console 面板。

点击「刷新扩展」按钮，Console 中应出现 `ic_list_extensions` 的调用记录（无报错）。

---

## 第六步：验证工具授权按钮

> 目标：确认 Agent 请求工具授权时，聊天界面出现「批准/拒绝」按钮，点击后能正确响应。

这个功能需要 Agent 执行一个需要审批的工具。有两种验证方式：

### 方式 A：开发者工具模拟（推荐，无需配置工具）

打开 Tauri 开发者工具（`Cmd+Option+I`），在 Console 中执行：

```javascript
window.__TAURI__.event.emit('chat-event', {
  type: 'approval_needed',
  request_id: 'test-req-001',
  tool_name: 'execute_shell',
  description: '执行命令: ls -la /tmp'
})
```

**预期**：聊天界面最后一条 assistant 消息底部出现授权卡片：
- 橙色边框，带盾牌图标
- 显示「工具执行需要授权：`execute_shell`」
- 显示描述文字「执行命令: ls -la /tmp」
- 两个按钮：「✓ 批准」和「✕ 拒绝」

### 6.1 测试批准

点击「批准」按钮。

**预期**：
- 授权卡片消失
- Console 中出现 `ic_approve_tool` 调用记录，无报错

### 6.2 测试拒绝

重新在 Console 执行上面的 emit 命令（换一个 request_id）：

```javascript
window.__TAURI__.event.emit('chat-event', {
  type: 'approval_needed',
  request_id: 'test-req-002',
  tool_name: 'write_file',
  description: '写入文件: /etc/hosts'
})
```

点击「拒绝」按钮。

**预期**：
- 授权卡片消失
- Console 中出现 `ic_deny_tool` 调用记录，无报错

### 6.3 测试多个并发授权请求

连续执行两次 emit（不同 request_id）：

```javascript
window.__TAURI__.event.emit('chat-event', {
  type: 'approval_needed',
  request_id: 'test-req-003',
  tool_name: 'tool_a',
  description: '操作 A'
})
window.__TAURI__.event.emit('chat-event', {
  type: 'approval_needed',
  request_id: 'test-req-004',
  tool_name: 'tool_b',
  description: '操作 B'
})
```

**预期**：同时显示两张授权卡片，分别批准/拒绝后各自消失，互不影响。

### 方式 B：后台配合验证完整审批流（可选）

如果想验证客户端授权与管理后台审批工单的联动：

**步骤 1**：在客户端 Console 执行 `submit_approval_ticket`：

```javascript
window.__TAURI__.core.invoke('submit_approval_ticket', {
  content: '申请导出用户数据报告',
  threadId: 'test-thread-001'
}).then(ticketId => {
  console.log('工单已创建，ticket_id:', ticketId)
})
```

**步骤 2**：打开管理后台 http://localhost:5174，进入左侧「审批工单」页面。

**预期**：列表中出现刚创建的工单，状态为「待审批」，申请内容为「申请导出用户数据报告」。

**步骤 3**：点击工单右侧「审批」按钮，选择「批准」，填写审批意见（可选），点击确认。

**步骤 4**：回到客户端，等待约 30 秒（轮询间隔）。

**预期**：客户端 Console 出现 `chat-event` 事件，type 为 `approval_result`，status 为 `approved`。

---

## 第七步：WebdriverIO E2E 测试（真实 Tauri 环境）

> ⚠️ **平台限制**：`tauri-driver` 仅支持 **Linux** 和 **Windows**。macOS 上通过 **Lima**（轻量 Linux VM）在本地运行真实测试。

### 方案 A：macOS 本地（Lima VM）

Lima 是专为 macOS 设计的轻量 Linux VM，Apple Silicon 原生支持，启动约 5 秒。

**安装 Lima（一次性）**：

```bash
brew install lima
```

**首次创建 VM（约 3-5 分钟）**：

```bash
cd desktop-client/e2e
npm run lima:start
```

**运行测试**：

```bash
npm run test:lima              # 全部测试
npm run test:lima:routines     # 仅定时任务
npm run test:lima:logs         # 仅日志
```

脚本会自动：
1. 检查/启动 Lima VM
2. 在 VM 内启动 `Xvfb` 虚拟显示
3. 编译 Linux 版 Tauri 应用
4. 运行 WebdriverIO 测试

**VM 管理**：

```bash
npm run lima:shell    # 进入 VM shell
npm run lima:stop     # 停止 VM（释放内存）
```

### 方案 B：Linux CI（GitHub Actions）

```yaml
- name: Install dependencies
  run: |
    sudo apt-get install -y webkit2gtk-driver xvfb libwebkit2gtk-4.1-dev

- name: Install tauri-driver
  run: cargo install tauri-driver --locked

- name: Run E2E tests
  run: xvfb-run npm test
  working-directory: desktop-client/e2e
```

### 预期结果

```
定时任务面板
  ✓ 点击定时任务按钮应打开面板
  ✓ 填写名称和描述后创建按钮应可点击
  ✓ 创建手动触发任务后应出现在列表中
  ✓ 创建 Cron 触发任务后应出现在列表中
  ✓ 点击取消应关闭创建弹窗且不创建任务
  ✓ 只填名称不填描述时创建按钮应为 disabled
  ✓ 点击开关应切换任务启用状态
  ✓ 任务卡片应显示执行历史元信息
  ✓ 点击已启用筛选应只显示启用的任务

日志 Tab
  ✓ 打开日志 Tab 应显示引擎启动日志
  ✓ 搜索框输入关键词应过滤日志
  ✓ 选择错误级别应只显示 error 日志
  ✓ 点击清空后确认应清空日志列表
  ✓ 点击导出应弹出系统保存对话框
```

---



**日志 Tab 仍然是空的**

引擎可能还在启动中。等待 10 秒后在 Console 执行：
```javascript
window.__TAURI__.core.invoke('ic_get_logs', { limit: 5 }).then(console.log)
```
如果返回空数组，检查 `/tmp/tauri.log` 确认引擎是否正常启动。

**定时任务执行历史不更新**

`ic_routine_runs` 从数据库读取，需要任务实际执行过。确认点击「运行」后等待任务完成（状态从 Running 变为 Completed）再重新打开面板。

**授权卡片不出现**

确认 Console 中 emit 命令执行成功（无报错）。如果有报错 `__TAURI__ is not defined`，说明 Tauri API 未注入，需要在 Tauri 窗口内的 Console 执行，而不是浏览器 Console。

**刷新扩展后列表为空**

正常现象，说明当前没有已安装的扩展。可以通过 `ic_search_extensions` 搜索可用扩展后安装。
