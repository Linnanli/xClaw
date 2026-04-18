/**
 * P2 E2E - Tauri IPC + 工具渲染
 *
 * 目标：在浏览器环境中模拟 Tauri IPC，验证 P2 工具卡片
 * （plan_mode / session_fork / sub_agent）能够通过历史与事件正确渲染。
 */

describe('P2 Chat Tool Rendering with Tauri IPC', () => {
  it('应从持久化 tool_calls 历史渲染 Plan/Fork/Sub-Agent 卡片', () => {
    const threadId = 'thread-p2-all';
    const now = '2025-01-01T00:00:00.000Z';
    const nowForSidebar = new Date().toISOString();

    cy.installTauriMock({
      threads: [
        {
          id: threadId,
          title: 'P2 全量工具卡片',
          started_at: nowForSidebar,
          last_activity: nowForSidebar,
        },
      ],
      threadHistoryById: {
        [threadId]: [
          {
            id: 'msg-user-1',
            role: 'user',
            content: '请先给我执行计划，再分叉并启动子 Agent',
            created_at: now,
          },
          {
            id: 'msg-tools-1',
            role: 'tool_calls',
            content: JSON.stringify({
              tool_calls: [
                {
                  id: 'call-plan-1',
                  name: 'plan_mode',
                  arguments: {
                    action: 'submit',
                    plan: {
                      goal: '完成 P2 验证',
                      steps: [
                        { description: '补测试', tool_name: 'cargo test', risk: 'low', files: ['desktop-client/ironclaw/tests'] },
                      ],
                      confidence: 0.86,
                    },
                  },
                  result: JSON.stringify({
                    action: 'submit',
                    goal: '完成 P2 验证',
                    steps_count: 1,
                    steps: [
                      { step: 1, description: '补测试', tool_name: 'cargo test', risk: 'low', files: ['desktop-client/ironclaw/tests'] },
                    ],
                    confidence: 0.86,
                  }),
                  status: 'completed',
                },
                {
                  id: 'call-fork-1',
                  name: 'session_fork',
                  arguments: { at_turn: 2, reason: '尝试另一条修复路径' },
                  result: JSON.stringify({
                    action: 'fork',
                    at_turn: 2,
                    reason: '尝试另一条修复路径',
                    new_thread_id: 'thread-branch-2',
                  }),
                  status: 'completed',
                },
                {
                  id: 'call-agent-1',
                  name: 'sub_agent',
                  arguments: {
                    role: 'verify',
                    goal: '验证 P2 质量门禁',
                    max_turns: 2,
                  },
                  result: JSON.stringify({
                    action: 'spawn',
                    role: 'verify',
                    goal: '验证 P2 质量门禁',
                    tool_whitelist: ['read_file', 'grep_search'],
                    max_turns: 2,
                    depth: 1,
                  }),
                  status: 'completed',
                },
              ],
            }),
            created_at: now,
          },
          {
            id: 'msg-assistant-1',
            role: 'assistant',
            content: '已生成计划并执行分叉与子 Agent 验证。',
            created_at: now,
          },
        ],
      },
    });

    cy.contains('P2 全量工具卡片').click();

    cy.contains('Execution Plan').should('be.visible');
    cy.contains('Session Fork').should('be.visible');
    cy.contains('Sub-Agent').should('be.visible');
  });

  it('应从历史回放渲染 Session Fork 关键字段', () => {
    const threadId = 'thread-p2-fork';
    const now = '2025-01-01T00:00:00.000Z';
    const nowForSidebar = new Date().toISOString();

    cy.installTauriMock({
      threads: [
        {
          id: threadId,
          title: 'P2 Session Fork',
          started_at: nowForSidebar,
          last_activity: nowForSidebar,
        },
      ],
      threadHistoryById: {
        [threadId]: [
          {
            id: 'msg-fork-tool-calls',
            role: 'tool_calls',
            content: JSON.stringify({
              tool_calls: [
                {
                  id: 'fork-call-1',
                  name: 'session_fork',
                  arguments: { at_turn: 3, reason: '回到关键拐点重试' },
                  result: JSON.stringify({
                    action: 'fork',
                    at_turn: 3,
                    reason: '回到关键拐点重试',
                    new_thread_id: 'thread-fork-from-history',
                  }),
                  status: 'completed',
                },
              ],
            }),
            created_at: now,
          },
        ],
      },
    });

    cy.contains('P2 Session Fork').click();

    cy.contains('Session Fork', { timeout: 8000 }).should('be.visible');
    cy.contains('from turn 3').should('be.visible');
  });

  it('应从历史回放渲染 Sub-Agent 角色与目标', () => {
    const threadId = 'thread-p2-subagent';
    const now = '2025-01-01T00:00:00.000Z';
    const nowForSidebar = new Date().toISOString();

    cy.installTauriMock({
      threads: [
        {
          id: threadId,
          title: 'P2 Sub-Agent',
          started_at: nowForSidebar,
          last_activity: nowForSidebar,
        },
      ],
      threadHistoryById: {
        [threadId]: [
          {
            id: 'msg-agent-tool-calls',
            role: 'tool_calls',
            content: JSON.stringify({
              tool_calls: [
                {
                  id: 'sub-agent-call-1',
                  name: 'sub_agent',
                  arguments: {
                    role: 'verify',
                    goal: '检查新增测试与文档是否齐备',
                    max_turns: 3,
                  },
                  result: JSON.stringify({
                    action: 'spawn',
                    role: 'verify',
                    goal: '检查新增测试与文档是否齐备',
                    tool_whitelist: ['read_file', 'grep_search', 'get_errors'],
                    max_turns: 3,
                    depth: 1,
                  }),
                  status: 'completed',
                },
              ],
            }),
            created_at: now,
          },
        ],
      },
    });

    cy.contains('P2 Sub-Agent').click();

    cy.contains('Sub-Agent', { timeout: 8000 }).should('be.visible');
    cy.contains('Verify').should('be.visible');
    cy.contains('检查新增测试与文档是否齐备').should('be.visible');
  });
});
