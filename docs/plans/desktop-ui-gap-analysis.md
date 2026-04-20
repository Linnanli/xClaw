# Desktop Client 前端交互查漏补缺计划

> 基于 opencode (anomalyco/opencode)、assistant-ui (assistant-ui/assistant-ui)、Cline (cline/cline) 三个开源仓库的对比研究生成。

## 一、已修复的 Bug

### Bug-1: MessagePrimitive.Parts API 误用（已修复）

**问题**：`thread.tsx` 使用 children-as-function 模式渲染 Parts，跳过了 assistant-ui 内部的分组逻辑。`ReasoningGroup` 从未被调用，reasoning 文本以纯 `<MarkdownText />` 渲染，没有折叠脑图标和自动展开行为。

**修复**：切换到 `components` prop API，传入 `Reasoning` + `ReasoningGroup`。

**影响**：
- 思考链现在有折叠 UI，流式时自动展开
- 消息 status = "running" 时 assistant-ui 自动显示 typing indicator
- ToolFallback 自动被 runtime 注册的 tool UI 覆盖

---

## 二、当前能力矩阵

### 2.1 X-Claw vs OpenCode vs Cline 功能对比

| 能力 | X-Claw 现状 | OpenCode | Cline | 差距 |
|------|-----------|----------|-------|------|
| **思考链展示** | ✅ 已修复 (ReasoningGroup) | ✅ ReasoningPartDisplay | ✅ ThinkingRow + shimmer | 无 |
| **流式文本** | ✅ stream_chunk + setMessages | ✅ PacedMarkdown (24ms pace) | ✅ partial 标记 + 自动滚动 | 缺 pacing 平滑动画 |
| **Loading indicator** | ✅ 已修复 (isRunning → typing indicator) | ✅ TextShimmer | ✅ Loading 动画 | 无 |
| **工具折叠展示** | ✅ CollapsibleToolShell | ✅ BasicTool + Accordion | ✅ Collapsible | 无 |
| **工具审批** | ✅ ApprovalCard (approve/deny) | ✅ PermissionCard (once/always/reject) | ✅ OptionsButtons (askResponse) | 无 |
| **Diff 展示** | ⚠️ 有但无交互 | ✅ @pierre/diffs | ✅ DiffEditRow + FileBlock | 缺 Undo |
| **Shell 输出** | ⚠️ 有但无交互 | ✅ BasicTool + 折叠 | ✅ CommandOutputRow (5行预览 + 展开) | 缺预览折叠 |
| **Context 工具分组** | ❌ 无 | ✅ ContextToolGroup (read/glob/grep/list 合并) | ❌ 无 | 缺分组 |
| **文件编辑 Auto-Apply + Undo** | ❌ 无 | ❌ 无（消息级 revert） | ✅ 有（工具级） | **P0 缺失** |
| **addResult 交互回路** | ❌ 未使用 | ✅ replyToPermission/Question | ✅ askResponse | **P0 缺失** |
| **消息级 Revert** | ❌ 无 | ✅ fork + revert | ❌ 无 | P2 缺失 |
| **流式 Pacing** | ❌ 无 | ✅ createPacedValue (逐字符) | ❌ 无 | P3 美化 |
| **ToolStep 时间线** | ✅ ToolStepIndicator | ❌ 无（用 Part 本身） | ❌ 无（用 ChatRow） | X-Claw 领先 |
| **DLP 实时扫描** | ✅ scanUserInput + fail-safe | ❌ 无 | ❌ 无 | X-Claw 领先 |
| **多模型切换** | ✅ ModelSelector | ✅ 模型选择 | ❌ 无 | X-Claw 领先 |

### 2.2 Tool Renderer 缺失能力

| Renderer | 当前能力 | 缺失能力 | 优先级 |
|----------|---------|---------|-------|
| file-edit-renderer | Diff 只读展示 | Undo 按钮（默认自动应用） | P0 |
| shell-output-renderer | 终端输出 + exit code | 5行预览 + 展开 | P2 |
| grep-result-renderer | 分组高亮展示 | 结果条目点击跳转（打开文件） | P1 |
| glob-result-renderer | 文件列表 | 文件条目点击打开 | P1 |
| plan-renderer | 步骤卡片 | 步骤确认/修改交互 | P2 |
| 所有 renderer | 无 addResult | addResult 反馈回 runtime | P0 |

---

## 三、实施计划

### Phase 1: 工具交互回路（P0，预计改动量最大）

**目标**：让用户对工具输出有操作能力，操作结果能反馈回 agent。

#### 1.1 共享交互基础设施

**参考**：opencode 的 `withOpenCodeToolInteractions` HOC 模式 + Cline 的 `askResponse` 模式

**改动文件**：
- `tool-ui-shared.tsx` — 添加 `ToolActionBar` 共享组件
- `TauriRuntimeProvider.tsx` — 添加 Tauri 命令调用辅助函数

**设计**：
```
CollapsibleToolShell
  ├─ 触发器（现有）
  ├─ 内容区（现有）
  └─ ToolActionBar（新增）
      └─ Undo 按钮 → invoke('undo_file_edit', { path }) — 恢复备份
```

#### 1.2 File Edit Auto-Apply + Undo

**设计决策**：文件编辑默认自动应用（当前行为），用户看到 Diff 后若不满意可点击 Undo 撤回。
不采用 Apply/Reject 模式，原因：(1) 多数情况用户接受编辑，Apply 变成多余点击；(2) 与当前 agent 直接写入文件的行为保持一致。

**参考**：Cline 的 `DiffEditRow.tsx`（Diff 展示） + opencode 的消息级 revert（撤回概念）

**改动文件**：
- `file-edit-renderer.tsx` — 添加 Undo 按钮
- `desktop-client/src/ipc/tools.rs`（新增）— `undo_file_edit` Tauri 命令

**交互流程**：
```
1. Agent 调用 code_edit 工具 → Rust 端写入文件（并自动备份原文件）
2. 前端渲染 Diff 视图 + Undo 按钮
3a. 用户满意 → 不操作，Diff 自然折叠
3b. 用户不满意 → 点击 Undo → invoke('undo_file_edit') → Rust 恢复备份 → addResult('undone by user')
4. Agent 收到 undo tool result，可选择重新编辑
```

**Rust 端 Tauri 命令**：
```rust
#[tauri::command]
async fn undo_file_edit(path: String) -> Result<(), String> {
    // 1. 从备份恢复原文件
    // 2. 清理备份
    // 3. 返回成功
}
```

> **未来可选**：在设置中增加 "Review Mode" 开关，启用后改为 Apply/Reject 模式（需用户手动确认才写入文件）。当前默认 auto-apply。

### Phase 2: Context 工具分组 + 结果导航（P1）

**目标**：减少视觉噪声，让 read/glob/grep/list 工具自动合并为一个折叠组。

#### 2.1 ContextToolGroup 组件

**参考**：opencode 的 `ContextToolGroup`（CONTEXT_GROUP_TOOLS set + groupParts 逻辑）

**改动文件**：
- `tool-renderers/context-group-renderer.tsx`（新增）— 将连续 context 类工具合并为带计数的折叠组
- `thread.tsx` — 在 `toolSteps` 渲染区域集成分组逻辑

**设计**：
```
┌─ 📚 已收集上下文（6 项）                    ▸
│  read: 3 个文件  |  grep: 2 次搜索  |  list: 1 个目录
└──────────────────────────────────────────────
```

#### 2.2 搜索结果点击跳转

**改动文件**：
- `grep-result-renderer.tsx` — 文件路径 + 行号可点击，调用 Tauri 命令打开文件
- `glob-result-renderer.tsx` — 同上
- `desktop-client/src/ipc/tools.rs` — `open_file_at_line` Tauri 命令

### Phase 3: 流式体验优化（P2）

**目标**：更平滑的流式文本输出和更好的错误反馈。

#### 3.1 文本 Pacing

**参考**：opencode 的 `createPacedValue()`

**改动文件**：
- `markdown-text.tsx` 或新增 `paced-text.tsx` — 逐词渲染而非直接 setState
- 自定义 hook `usePacedValue(text, isStreaming)` — 24ms 间隔，在单词边界停留

**设计决策**：
- 仅在 streaming 状态激活 pacing，完成后立即显示全部
- 不影响 reasoning 和 tool output（它们不需要 pacing）

#### 3.2 消息级 Revert

**参考**：opencode 的 `fork` + `revert` 操作

**改动文件**：
- `thread.tsx` AssistantActionBar — 添加 "撤回到此处" 按钮
- `TauriRuntimeProvider.tsx` — `revertToMessage(messageId)` 实现

#### 3.3 Shell 输出预览模式

**参考**：Cline 的 `CommandOutputRow`（5行预览 + 展开）

**改动文件**：
- `shell-output-renderer.tsx` — 默认显示 5 行，超出部分需点击展开

---

## 四、不实施的能力（已评估，不符合当前架构）

| 能力 | 来源 | 不实施原因 |
|------|------|-----------|
| Shell Rerun 按钮 | Cline | X-Claw 没有嵌入式终端界面，Rerun 无处承载；用户可通过对话让 agent 重新执行 |
| PacedMarkdown (Solid.js) | opencode | opencode 用 Solid.js 实现，X-Claw 是 React；可借鉴思路但需用 React hook 重写 |
| @pierre/diffs | opencode | 当前 DiffView 已满足基本需求，引入新库 ROI 不高 |
| gRPC 通信协议 | Cline | X-Claw 已有 Tauri IPC，gRPC 是 Cline 的 VS Code extension 专用 |
| 文件树 / Workspace 面板 | Cursor/Copilot | 需要 Tauri 侧重度集成，属于独立大功能，不在本轮范围 |
| 自动审批策略面板 | Cline | X-Claw 审批走工单系统，无需本地策略面板 |
| 工单审批 UI (submitForReview) | X-Claw 自有 | 优先级不高，已从前端移除；后续按需从 Rust 端恢复 |

---

## 五、依赖关系和实施顺序

```
Phase 1.1 (共享基础设施)
    │
    └──→ Phase 1.2 (File Edit Auto-Apply + Undo)
            └──→ 需要新增 Rust Tauri 命令 undo_file_edit

Phase 2.1 (ContextToolGroup)
    └──→ Phase 2.2 (搜索结果点击跳转)
            └──→ 需要新增 Rust Tauri 命令 open_file_at_line

Phase 3 (流式优化) — 独立可并行
```

---

## 六、技术风险

| 风险 | 等级 | 缓解措施 |
|------|------|---------|
| addResult 流程与 ironclaw agent 不兼容 | 中 | 先验证 useExternalStoreRuntime 的 addResult 是否能注入 tool result 到 Rust 端 agent 消息流 |
| undo_file_edit 备份竞态 | 中 | 备份文件命名包含时间戳，避免并发编辑覆盖 |
| ContextToolGroup 与 toolSteps 冲突 | 低 | toolSteps 是流式过程指示器，ContextToolGroup 是完成后的结构化展示，不矛盾 |

---

## 七、验收标准

### Phase 1 验收
- [ ] 文件编辑默认自动应用，Diff 视图下方有 Undo 按钮
- [ ] 点击 Undo 成功恢复原文件，agent 收到 "undone by user" tool result
- [ ] 备份文件正确创建和清理
- [ ] 所有交互走 DLP + SafetyBridge，无安全绕过

### Phase 2 验收
- [ ] 连续 read/glob/grep 工具自动合并为折叠组
- [ ] grep 结果条目可点击跳转到文件
- [ ] glob 结果条目可点击打开文件

### Phase 3 验收
- [ ] 流式文本有 pacing 动画，不再一次性跳出大段文字
- [ ] Shell 输出默认 5 行预览，超出可展开
- [ ] 消息级 "撤回到此处" 功能可用

---

## 八、参考仓库索引

| 仓库 | 本地路径 | 关键文件 |
|------|---------|---------|
| opencode | `/tmp/x-claw-refs/opencode` | `packages/ui/src/components/message-part.tsx`, `basic-tool.tsx`, `session-turn.tsx`, `session-diff.ts` |
| assistant-ui | `/tmp/x-claw-refs/assistant-ui` | `examples/with-opencode/components/tools/`, `packages/react-opencode/src/`, `examples/with-external-store/` |
| Cline | `/tmp/x-claw-refs/cline` | `webview-ui/src/components/chat/DiffEditRow.tsx`, `CommandOutputRow.tsx`, `ThinkingRow.tsx`, `OptionsButtons.tsx` |
