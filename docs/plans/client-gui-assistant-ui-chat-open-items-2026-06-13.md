# client-gui assistant-ui PRD 未完成清单

日期：2026-06-13

来源 PRD：`.omx/plans/prd-client-gui-assistant-ui-chat.md`

结论：当前实现已从第一阶段骨架推进到 **assistant-ui adapter 边界基本完成**。`client-gui` 已接入 `@assistant-ui/react`，并完成 runtime/thread/message/action/composer/input/attachment/suggestion/reasoning-tool adapter/model-selector 的主要路径；Browser renderer smoke 已覆盖 WelcomeView 的 composer/suggestion surface；但按 PRD 全量验收标准，仍缺 chat/send/stop/streaming/attachment 的完整 App smoke 与验收矩阵。

## 核验范围

- 已用语义搜索检查是否已有等价 open-items 清单：未发现。
- 已用精确搜索核对 `client-gui/src/renderer`、`client-gui/tests`、`client-gui/package.json` 与 PRD 关键项。
- 当前会话未暴露 LSP `execute_lsp` 工具，因此符号层验证未执行；本清单的否定结论基于语义搜索与精确搜索双证据。

## 已完成基线

- 依赖已接入：`client-gui/package.json` 中已有 `@assistant-ui/react@0.12.28`，并通过 overrides 锁定 `@assistant-ui/store` / `@assistant-ui/tap`。
- primitive coverage matrix 已存在：`client-gui/src/renderer/components/assistant-ui/primitiveCoverage.ts`。
- runtime adapter 已存在：`AssistantRuntimeProvider + useExternalStoreRuntime` 位于 `client-gui/src/renderer/components/assistant-ui/runtime/AssistantRuntimeAdapter.tsx`。
- chat 主路径已接入 assistant-ui shell：`ChatView.tsx` 使用 `AssistantChatShell`、`AssistantThreadView`、`AssistantComposer`、`AssistantModelSelector`。
- `ThreadPrimitive`、`MessagePrimitive.Root`、`ActionBarPrimitive.Copy`、`ComposerPrimitive.Root`、`ComposerPrimitive.Input`、`AttachmentPrimitive.Root`、`ThreadPrimitive.Suggestion`、`ChainOfThoughtPrimitive.Root` 已落地或以 documented adapter 方式落地。
- MCP connector count 已从 chat header 移除。

## 为什么接入 assistant-ui 后客户端看起来仍像旧版

证据与推论：

- `ChatView.tsx` 原先只是把旧 header、旧 max-width message area、旧 composer 卡片样式包进 `AssistantRuntimeAdapter`，视觉 token 和布局仍由 `globals.css` / 旧 Tailwind class 决定。
- `ComposerSurface.tsx` 原先只用了 `ComposerPrimitive.Root`，输入仍是受控 `<textarea>`，附件仍是旧 `ComposerAttachmentTray`；因此 assistant-ui 没有接管输入或附件视觉。
- `AssistantContentBlock.tsx` 原先只是把所有 block 转交旧 `ContentBlockView`，thinking/tool UI 仍完全沿用旧渲染。
- `WelcomeView.tsx` 原先 quick tags 是普通 `<button>`，没有进入 assistant-ui suggestion surface。
- assistant-ui 是 headless primitives，默认不会自动提供一套成品视觉皮肤；如果继续复用旧 className，外观自然保持旧样子。

本轮修复后的变化：

- 新增 `AssistantChatShell`，把 chat 页面壳、runtime、thread、composer 边界从 `ChatView` 中抽出。
- 新增 `AssistantComposer`，使用 `ComposerPrimitive.Input asChild` 适配受控 prompt，同时保留 IME-safe Enter、paste/drop、disabled/submit 逻辑。
- 新增 `AssistantAttachmentTray`，用 `AttachmentPrimitive.Root` 包裹本地草稿附件；`AttachmentPrimitive.Name/Remove` 因依赖 assistant-ui attachment context，暂不强行使用。
- 新增 `AssistantSuggestionList`，用 `ThreadPrimitive.Suggestion` 适配 welcome quick tags。
- `AssistantContentBlock` 对 thinking/tool_use/tool_result 输出 assistant-ui adapter metadata，并用 `ChainOfThoughtPrimitive.Root` 包裹 reasoning。
- `AssistantRuntimeAdapter` 修复了 `tool_use` / `tool_result` 同 id 转成重复 `tool-call` 的问题：配对 result 现在合并进 tool-call runtime part，孤儿 result 降级为 text part。

## 未完成清单

| ID    | 状态     | PRD 要求                                                                                                                                                                                                                       | 当前证据                                                                                                                                                                                                                                                   | 待完成内容                                                                                                                                     |
| ----- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| AU-01 | 已完成   | composer 使用 assistant-ui composer/action primitives，尽量覆盖 input 和 attachments。                                                                                                                                         | `AssistantComposer.tsx` 使用 `ComposerPrimitive.Root` + `ComposerPrimitive.Input asChild`；`composer-surface-behavior.test.ts` 保留 IME-safe Enter helper；`assistant-ui-runtime-behavior.test.tsx` 覆盖受控 submit 不误触 runtime `onNew`。               | 后续若 assistant-ui 能完全接管 draft state，再评估移除受控 prompt adapter。                                                                    |
| AU-02 | 部分完成 | 附件展示/草稿预览使用 assistant-ui attachment surface 或 documented adapter。                                                                                                                                                  | `AssistantAttachmentTray.tsx` 使用 `AttachmentPrimitive.Root` 包裹 pasted image / attached file；DOM 测试覆盖本地 remove handler。                                                                                                                         | 仍需补 dropped image、selected file、dropped non-image file 的端到端 DOM/IPC 覆盖；`AttachmentPrimitive.Name/Remove` 暂因 context 归属未采用。 |
| AU-03 | 已完成   | thinking/reasoning 和 tool/tool result 使用 assistant-ui reasoning/tool display 或清晰 adapter。                                                                                                                               | `AssistantContentBlock.tsx` 显式区分 reasoning/tool-use/tool-result adapter；`assistant-ui-runtime-behavior.test.tsx` 覆盖 thinking、tool use、merged tool result、orphan error result；runtime adapter 已修复 duplicate `toolCallId`。                    | 可后续接入更完整的 `MessagePrimitive.Parts` / registered tool UI，但当前语义已显式 adapter 化。                                                |
| AU-04 | 已完成   | welcome suggestions 接近 assistant-ui/base.tsx suggestion cards，可使用 `SuggestionPrimitive`。                                                                                                                                | `AssistantSuggestionList.tsx` 使用 `ThreadPrimitive.Suggestion`；`WelcomeView.tsx` quick tags 已进入 runtime provider；DOM 测试覆盖点击填充 prompt。                                                                                                       | `SuggestionPrimitive.Trigger` 依赖 suggestion context，不适合当前 quick tag 本地数据源；已在 coverage matrix 记录。                            |
| AU-05 | 已完成   | 按 PRD 边界拆出 assistant-ui 组件区：`AssistantChatShell`、`AssistantThreadView`、`AssistantMessage`、`AssistantContentBlock`、`AssistantComposer`、`AssistantAttachmentTray`、`AssistantModelSelector`、`assistantAdapters`。 | 上述边界均已存在并从 `assistant-ui/index.ts` 导出；旧 `ComposerSurface` 仅保留兼容 re-export。                                                                                                                                                             | 无。                                                                                                                                           |
| AU-06 | 已完成   | 模型选择器需要证明 config-backed 与 dasclaw app-server 两路选择、committed event round trip、错误展示。                                                                                                                        | `use-client-gui-model-selector-contract.test.tsx` 真实渲染 `AssistantModelSelector`，覆盖 app-server select、config fallback save、selection error global notice；`use-ipc-model-provider-event.test.tsx` 通过真实 listener 发送 `modelProvider.changed`。 | 可后续补视觉 smoke。                                                                                                                           |
| AU-07 | 已完成   | 测试应迁移为行为/交互契约，不能用测试侧适配掩盖客户端 bug。                                                                                                                                                                    | assistant-ui/model selector 相关源码字符串测试已替换为 DOM/hook 行为测试；`vitest.config.mts` 已纳入 `tsx` 测试，避免行为测试文件未执行。                                                                                                                  | 仍可逐步清理其他非本 PR 范围的源码字符串测试。                                                                                                 |
| AU-08 | 部分完成 | 行为覆盖 send、stop、attachment conversion、streaming partial placement、tool/thinking rendering、model switching、welcome submission。                                                                                        | 本轮覆盖 composer submit、attachment tray、suggestion fill、thinking/tool adapter、model switching、IPC event roundtrip、ChatView shell DOM；既有测试仍覆盖部分 stop/streaming/helper。                                                                    | 仍需建立完整 PRD 验收矩阵，尤其是 welcome start、streaming partial placement、timer、drop/file picker 的端到端路径。                           |
| AU-09 | 部分完成 | Browser/App smoke check 覆盖 chat、welcome、model selector、send、stop、attachment preview、streaming。                                                                                                                        | `npx --prefix client-gui vite --host 127.0.0.1 --port 5174` + in-app Browser 已验证 WelcomeView 非空、无 Vite overlay、`[data-assistant-composer="client-gui"]` 与 `[data-assistant-suggestions]` 存在。                                                   | 仍需补完整 App smoke：chat session shell、model selector 视觉、send、stop、attachment preview、streaming。                                     |
| AU-10 | 已完成   | primitive coverage matrix 需要随实现继续维护。                                                                                                                                                                                 | `primitiveCoverage.ts` 已更新 runtime/thread/message/action/composer input/attachment/suggestion/reasoning rows，标清 chosen implementation、fallback、canonical state read。                                                                              | 后续新增 primitives 时继续维护。                                                                                                               |

## 剩余修复顺序

1. **补完整 PRD 验收矩阵**：把 welcome start、streaming partial placement、timer、drop/file picker、stop 的端到端路径补成 DOM/IPC 级测试。
2. **补完整 App smoke 证据**：在真实桌面客户端或可注入会话状态的 Browser harness 中覆盖 chat、model selector、send、stop、attachment preview、streaming，记录命令、环境、截图或关键日志。
3. **评估 assistant-ui 更深接管边界**：只有在能消除双状态风险时，才把 prompt / draft attachments 从 client-gui 本地 state 迁入 assistant-ui composer/attachment state。

## 验证状态

本轮已知验证结果：

- `npm --prefix client-gui run typecheck` 通过。
- `npm --prefix client-gui run lint` 通过，0 error；仍有 5 个既有 `react-hooks/exhaustive-deps` warning，位于 `App.tsx`、`MessageCard.tsx`、`SettingsPanel.tsx`、`useApiConfigState.ts`。
- `npm --prefix client-gui run test -- --run tests/assistant-ui-runtime-behavior.test.tsx tests/use-client-gui-model-selector-contract.test.tsx tests/use-ipc-model-provider-event.test.tsx tests/assistant-ui-contract.test.ts tests/composer-surface-behavior.test.ts tests/chat-view-model-selector.test.tsx` 通过，6 个文件 17 个测试。
- `npm --prefix client-gui run test -- --run tests/logger-fallback.test.ts` 通过，1 个文件 6 个测试。
- `npm --prefix client-gui run test -- --run` 通过，149 个文件 994 个测试。
- Browser renderer smoke：`http://127.0.0.1:5174/` 非空，`document.title` 为 `Open Cowork`，无 `vite-error-overlay`，WelcomeView 上 `data-assistant-composer="client-gui"` 与 `data-assistant-suggestions` 存在。

这些验证说明本轮 adapter 边界、model selector roundtrip、composer/attachment/suggestion/tool-thinking 行为已被锁定；仍不等于 PRD 全量验收完成，AU-08/AU-09 仍需补完整矩阵和真实 chat/send/stop/streaming App smoke。
