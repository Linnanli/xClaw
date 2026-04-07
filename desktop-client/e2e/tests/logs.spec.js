/**
 * 日志 Tab E2E 测试 — WebdriverIO + tauri-driver
 *
 * 测试真实 Tauri 应用的日志功能，包括加载、搜索、过滤、导出、清空。
 */

describe('日志 Tab', () => {
  async function waitForApp() {
    await $('button=日志').waitForDisplayed({ timeout: 30000 });
  }

  async function openLogsTab() {
    await $('button=日志').click();
    await $('button=导出').waitForDisplayed({ timeout: 5000 });
  }

  // ── 日志加载 ──────────────────────────────────────────────────

  it('打开日志 Tab 应显示引擎启动日志', async () => {
    await waitForApp();
    await openLogsTab();

    // 引擎启动后应有日志条目
    // 等待日志加载（最多 5 秒）
    await browser.waitUntil(
      async () => {
        const entries = await $$('.rounded-xl.border.border-border.bg-card');
        return entries.length > 0;
      },
      { timeout: 5000, timeoutMsg: '日志条目未在 5 秒内出现' }
    );

    const entries = await $$('.rounded-xl.border.border-border.bg-card');
    expect(entries.length).toBeGreaterThan(0);
  });

  // ── 搜索 ──────────────────────────────────────────────────────

  it('搜索框输入关键词应过滤日志', async () => {
    await waitForApp();
    await openLogsTab();

    // 等待日志加载
    await browser.waitUntil(
      async () => (await $$('.rounded-xl.border.border-border.bg-card')).length > 0,
      { timeout: 5000 }
    );

    const totalBefore = (await $$('.rounded-xl.border.border-border.bg-card')).length;

    // 搜索一个不太可能存在的词
    await $('input[placeholder="搜索日志..."]').setValue('xyznotexist12345');
    await browser.pause(500);

    // 应显示"没有找到匹配的日志"
    await expect($('*=没有找到匹配的日志')).toBeDisplayed();

    // 清空搜索，恢复全部
    await $('input[placeholder="搜索日志..."]').clearValue();
    await browser.pause(500);

    const totalAfter = (await $$('.rounded-xl.border.border-border.bg-card')).length;
    expect(totalAfter).toBe(totalBefore);
  });

  // ── 级别过滤 ──────────────────────────────────────────────────

  it('选择错误级别应只显示 error 日志', async () => {
    await waitForApp();
    await openLogsTab();

    await browser.waitUntil(
      async () => (await $$('.rounded-xl.border.border-border.bg-card')).length > 0,
      { timeout: 5000 }
    );

    // 选择"错误"级别
    await $('select').selectByVisibleText('错误');
    await browser.pause(500);

    // 所有可见日志条目应包含"错误"标签
    const entries = await $$('.rounded-xl.border.border-border.bg-card');
    for (const entry of entries) {
      const text = await entry.getText();
      // 每条 error 日志应包含"错误"标签文字
      expect(text).toContain('错误');
    }

    // 恢复
    await $('select').selectByVisibleText('所有级别');
  });

  // ── 清空 ──────────────────────────────────────────────────────

  it('点击清空后确认应清空日志列表', async () => {
    await waitForApp();
    await openLogsTab();

    await browser.waitUntil(
      async () => (await $$('.rounded-xl.border.border-border.bg-card')).length > 0,
      { timeout: 5000 }
    );

    // 点击清空按钮
    await $('button=清空').click();

    // 等待 ConfirmDialog 出现（shadcn AlertDialog）
    await $('button=确认清空').waitForDisplayed({ timeout: 3000 });
    await $('button=确认清空').click();

    // 日志列表应变空
    await browser.waitUntil(
      async () => (await $$('.rounded-xl.border.border-border.bg-card')).length === 0,
      { timeout: 3000, timeoutMsg: '清空后日志列表未变空' }
    );

    await expect($('*=没有找到匹配的日志')).toBeDisplayed();
  });

  // ── 导出 ──────────────────────────────────────────────────────

  it('点击导出应弹出系统保存对话框', async () => {
    await waitForApp();
    await openLogsTab();

    await browser.waitUntil(
      async () => (await $$('.rounded-xl.border.border-border.bg-card')).length > 0,
      { timeout: 5000 }
    );

    // 点击导出按钮
    // 注意：系统保存对话框是原生 UI，WebDriver 无法直接操作
    // 这里只验证点击不报错（不抛出异常）
    await $('button=导出').click();

    // 等待一小段时间，确认没有 JS 错误
    await browser.pause(1000);

    // 如果有错误提示出现，测试失败
    const errorEl = await $('*=导出失败');
    await expect(errorEl).not.toBeDisplayed();
  });
});
