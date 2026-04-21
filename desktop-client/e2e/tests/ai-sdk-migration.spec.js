/**
 * Phase 1 AI-SDK 迁移冒烟测试
 *
 * 对应 `docs/plans/architecture-refactor/03-phase1-ai-sdk-migration.md` Step H。
 *
 * 前置：
 *   - 运行时环境变量 `VITE_USE_AI_SDK_RUNTIME=true`（切到新 ChatRuntimeProvider）
 *   - 应用已编译（cargo build -p desktop-client）
 *   - 至少配置一个可用模型（Ollama 本地 或 DeepSeek 在线）
 *
 * 验收目标（来自 Phase 1 Step H 验收清单）：
 *   1. 文本流式渲染 —— 消息逐 token 更新
 *   2. 工具调用 ToolUI 正常渲染（shell / file_edit / web_search 等）
 *   3. Approval ToolUI 在需要审批的工具被触发时出现
 *   4. 点击"批准" → 工具继续执行 → 卡片切换为"已批准" banner
 *   5. 点击"拒绝" → 工具中止 → 卡片切换为"已拒绝" banner
 *   6. 全程不出现"审批卡片不可见" bug（旧 FloatingApprovalBanner 的顽疾）
 *   7. 切换 branch 不会吞掉 pending approval（SDK 按 toolCallId 挂到正确 branch）
 *
 * 状态：骨架（skeleton）。实际 $selector 需在真机调试时补齐。
 * 标记为 skip 避免误入 CI；手动验收前去掉 skip。
 */

describe.skip('Phase 1 AI SDK 迁移 — 冒烟测试', () => {
  async function waitForApp() {
    await $('button=聊天').waitForDisplayed({ timeout: 30000 });
  }

  async function sendMessage(text) {
    const composer = await $('textarea[placeholder*="输入"]');
    await composer.setValue(text);
    await browser.keys(['Meta', 'Enter']); // macOS Cmd+Enter；Linux/Win 用 Control+Enter
  }

  it('1. 文本流式渲染', async () => {
    await waitForApp();
    await sendMessage('你好');
    // 期望助手消息逐步出现
    const assistantMsg = await $('[data-role="assistant"] .aui-message-content');
    await assistantMsg.waitForDisplayed({ timeout: 10000 });
    await browser.waitUntil(
      async () => {
        const txt = await assistantMsg.getText();
        return txt.length > 0;
      },
      { timeout: 15000, timeoutMsg: '助手文本未流式输出' },
    );
  });

  it('2. 工具调用 ToolUI 渲染', async () => {
    await waitForApp();
    await sendMessage('读取 README.md 前 5 行');
    // 期望出现工具 UI（无论哪个具体 tool-renderer）
    await $('[data-testid*="tool-"]').waitForDisplayed({ timeout: 20000 });
  });

  it('3-5. Approval ToolUI 生命周期（批准路径）', async () => {
    await waitForApp();
    await sendMessage('请帮我删除 /tmp/nonexistent-file-for-e2e');

    // 3. 期望出现 approval 卡片
    const card = await $('[data-testid="approval-card"]');
    await card.waitForDisplayed({ timeout: 20000 });

    // 4. 点击批准
    await $('[data-testid="approval-approve"]').click();

    // 5. 期望切换为"已批准" banner（approved=true）
    const banner = await $('[data-testid="approval-result"][data-approved="true"]');
    await banner.waitForDisplayed({ timeout: 10000 });
  });

  it('3-5. Approval ToolUI 生命周期（拒绝路径）', async () => {
    await waitForApp();
    await sendMessage('请帮我删除 /tmp/another-nonexistent-file');

    const card = await $('[data-testid="approval-card"]');
    await card.waitForDisplayed({ timeout: 20000 });

    await $('[data-testid="approval-deny"]').click();

    const banner = await $('[data-testid="approval-result"][data-approved="false"]');
    await banner.waitForDisplayed({ timeout: 10000 });
  });

  it('6. 切换 branch 不吞 approval（回归测试）', async () => {
    await waitForApp();
    await sendMessage('请帮我删除 /tmp/branch-test-file');

    const card = await $('[data-testid="approval-card"]');
    await card.waitForDisplayed({ timeout: 20000 });

    // 尝试编辑上一条用户消息，产生新 branch
    const editBtn = await $('.aui-user-action-edit').$$('*').pop();
    if (editBtn && (await editBtn.isDisplayed())) {
      await editBtn.click();
      await browser.keys(['Escape']); // 取消编辑
    }

    // approval 卡片应仍在（挂在原 branch 上，不会被吞掉）
    await card.waitForDisplayed({ timeout: 2000 });
  });
});
