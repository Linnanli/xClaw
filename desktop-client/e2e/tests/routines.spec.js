/**
 * 定时任务 E2E 测试 — WebdriverIO + tauri-driver
 *
 * 测试真实 Tauri 应用，IPC 调用走真实 Rust 命令，不需要 mock。
 *
 * 前置条件：
 *   - 应用已编译（cargo build -p desktop-client）
 *   - 数据库可用（libsql 本地文件，应用启动时自动创建）
 *   - 已通过密码登录（或应用处于已登录状态）
 */

describe('定时任务面板', () => {
  // ── 辅助函数 ──────────────────────────────────────────────────

  /** 等待应用主界面加载完成 */
  async function waitForApp() {
    // 等待侧边栏的"定时任务"按钮出现，说明应用已就绪
    await $('button=定时任务').waitForDisplayed({ timeout: 30000 });
  }

  /** 打开定时任务面板 */
  async function openRoutinesPanel() {
    await $('button=定时任务').click();
    await $('button=新建定时任务').waitForDisplayed({ timeout: 5000 });
  }

  /** 关闭定时任务面板 */
  async function closeRoutinesPanel() {
    const closeBtn = await $('[aria-label="Close"]');
    if (await closeBtn.isDisplayed()) {
      await closeBtn.click();
    }
  }

  /** 创建一个定时任务并返回任务名 */
  async function createRoutine(name, description, triggerType = '手动') {
    await $('button=新建定时任务').click();
    await $('input[placeholder="输入任务名称"]').waitForDisplayed({ timeout: 3000 });

    await $('input[placeholder="输入任务名称"]').setValue(name);
    await $('textarea[placeholder="输入任务描述"]').setValue(description);

    if (triggerType !== '手动') {
      await $('select').selectByVisibleText(triggerType === 'cron' ? '时间（Cron）' : '事件');
      if (triggerType === 'cron') {
        await $('input[placeholder="例如: 0 9 * * *"]').setValue('0 9 * * *');
      }
    }

    await $('button=创建').click();
    // 等待弹窗关闭
    await $('h3=新建定时任务').waitForDisplayed({ timeout: 3000, reverse: true });
    return name;
  }

  // ── 面板打开/关闭 ──────────────────────────────────────────────

  it('点击定时任务按钮应打开面板', async () => {
    await waitForApp();
    await $('button=定时任务').click();

    await expect($('button=新建定时任务')).toBeDisplayed();
    await expect($('button=全部任务')).toBeDisplayed();

    await closeRoutinesPanel();
  });

  // ── 新建任务 — 正常路径 ────────────────────────────────────────

  it('填写名称和描述后创建按钮应可点击', async () => {
    await waitForApp();
    await openRoutinesPanel();
    await $('button=新建定时任务').click();

    await $('input[placeholder="输入任务名称"]').waitForDisplayed({ timeout: 3000 });

    // 初始状态：创建按钮 disabled
    const createBtn = await $('button=创建');
    await expect(createBtn).toBeDisabled();

    // 填写名称和描述后应可点击
    await $('input[placeholder="输入任务名称"]').setValue('测试任务');
    await $('textarea[placeholder="输入任务描述"]').setValue('描述');
    await expect(createBtn).not.toBeDisabled();

    // 取消，不实际创建
    await $('button=取消').click();
    await closeRoutinesPanel();
  });

  it('创建手动触发任务后应出现在列表中', async () => {
    await waitForApp();
    await openRoutinesPanel();

    const taskName = `E2E手动任务_${Date.now()}`;
    await createRoutine(taskName, '自动化测试创建的手动任务');

    // 任务应出现在列表中
    await expect($(`=${taskName}`)).toBeDisplayed();

    await closeRoutinesPanel();
  });

  it('创建 Cron 触发任务后应出现在列表中', async () => {
    await waitForApp();
    await openRoutinesPanel();

    const taskName = `E2E定时任务_${Date.now()}`;
    await createRoutine(taskName, '每天9点执行', 'cron');

    await expect($(`=${taskName}`)).toBeDisplayed();

    await closeRoutinesPanel();
  });

  it('点击取消应关闭创建弹窗且不创建任务', async () => {
    await waitForApp();
    await openRoutinesPanel();

    // 记录当前任务数
    const countBefore = await $$('[role="switch"]').length;

    await $('button=新建定时任务').click();
    await $('input[placeholder="输入任务名称"]').setValue('不会创建的任务');
    await $('button=取消').click();

    // 弹窗关闭
    await expect($('h3=新建定时任务')).not.toBeDisplayed();

    // 任务数不变
    const countAfter = await $$('[role="switch"]').length;
    expect(countAfter).toBe(countBefore);

    await closeRoutinesPanel();
  });

  // ── 新建任务 — 失败路径 ────────────────────────────────────────

  it('只填名称不填描述时创建按钮应为 disabled', async () => {
    await waitForApp();
    await openRoutinesPanel();
    await $('button=新建定时任务').click();

    await $('input[placeholder="输入任务名称"]').setValue('只有名称');
    // 不填描述
    await expect($('button=创建')).toBeDisabled();

    await $('button=取消').click();
    await closeRoutinesPanel();
  });

  // ── 启用/禁用 ─────────────────────────────────────────────────

  it('点击开关应切换任务启用状态', async () => {
    await waitForApp();
    await openRoutinesPanel();

    // 先创建一个任务
    const taskName = `E2E开关测试_${Date.now()}`;
    await createRoutine(taskName, '用于测试开关');

    // 找到该任务的开关
    const taskRow = await $(`=${taskName}`).parentElement();
    const toggle = await taskRow.$('[role="switch"]');

    const initialChecked = await toggle.getAttribute('aria-checked');

    // 点击开关
    await toggle.click();
    await browser.pause(500); // 等待状态更新

    const newChecked = await toggle.getAttribute('aria-checked');
    expect(newChecked).not.toBe(initialChecked);

    await closeRoutinesPanel();
  });

  // ── 执行历史 ──────────────────────────────────────────────────

  it('任务卡片应显示执行历史元信息', async () => {
    await waitForApp();
    await openRoutinesPanel();

    // 确保有任务存在
    const taskName = `E2E历史测试_${Date.now()}`;
    await createRoutine(taskName, '用于测试执行历史');

    // 卡片应显示"上次执行"和"已执行"字段
    await expect($('*=上次执行：')).toBeDisplayed();
    await expect($('*=已执行')).toBeDisplayed();

    await closeRoutinesPanel();
  });

  // ── 筛选 ──────────────────────────────────────────────────────

  it('点击已启用筛选应只显示启用的任务', async () => {
    await waitForApp();
    await openRoutinesPanel();

    await $('button=已启用').click();
    await browser.pause(300);

    // 已禁用的任务不应显示（通过 switch 状态验证）
    const switches = await $$('[role="switch"]');
    for (const sw of switches) {
      const checked = await sw.getAttribute('aria-checked');
      expect(checked).toBe('true');
    }

    await closeRoutinesPanel();
  });
});
