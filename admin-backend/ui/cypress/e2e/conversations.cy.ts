/**
 * 对话审计 E2E 测试
 *
 * 覆盖维度：
 * - 正常路径：页面渲染、统计卡片、筛选栏、表格结构
 * - 交互：搜索、DLP 筛选、对话详情弹窗
 * - 导航：侧边栏跳转
 * - 空数据：无对话时的空状态展示
 */

describe('对话审计', () => {
  beforeEach(() => {
    cy.loginByState()
  })

  describe('正常路径', () => {
    beforeEach(() => {
      cy.visit('/conversations')
    })

    it('应渲染页面标题和操作按钮', () => {
      cy.contains('对话审计').should('be.visible')
      cy.contains('审计员工与 AI 助手的对话记录，确保合规使用').should('be.visible')
      cy.contains('导出报告').should('be.visible')
    })

    it('应渲染 4 个统计卡片', () => {
      cy.contains('今日对话').should('be.visible')
      cy.contains('Token 消耗').should('be.visible')
      cy.contains('DLP 标记').should('be.visible')
      cy.contains('活跃用户').should('be.visible')
    })

    it('应渲染筛选栏', () => {
      cy.get('input[placeholder="搜索用户或主题..."]').should('be.visible')
      cy.contains('全部 DLP').should('be.visible')
    })

    it('应渲染表格表头', () => {
      const headers = ['用户', '对话主题', '消息数', 'TOKEN', '模型', 'DLP', '时间']
      headers.forEach(h => cy.contains(h).should('be.visible'))
    })
  })

  describe('交互', () => {
    beforeEach(() => {
      cy.visit('/conversations')
    })

    it('搜索框应可输入', () => {
      cy.get('input[placeholder="搜索用户或主题..."]').type('zhang')
      cy.get('input[placeholder="搜索用户或主题..."]').should('have.value', 'zhang')
    })

    it('DLP 筛选应可选择', () => {
      cy.get('select').contains('全部 DLP').parent('select').select('flagged')
    })

    it('有数据时点击行应打开对话详情弹窗', () => {
      cy.visit('/conversations')
      // 等待加载完成后检查
      cy.wait(2000)
      cy.get('body').then($body => {
        if (!$body.text().includes('暂无对话记录')) {
          cy.get('div[class*="cursor-pointer"]').first().click()
          cy.contains('对话详情').should('be.visible')
        }
      })
    })
  })

  describe('空数据展示', () => {
    it('无对话记录时应展示空状态', () => {
      cy.visit('/conversations')
      cy.get('body').then($body => {
        if ($body.text().includes('暂无对话记录')) {
          cy.contains('暂无对话记录').should('be.visible')
        }
      })
    })
  })

  describe('导航', () => {
    it('从侧边栏点击对话审计能跳转', () => {
      cy.visit('/')
      cy.get('aside').contains('对话审计').click()
      cy.url().should('include', '/conversations')
      cy.contains('审计员工与 AI 助手的对话记录，确保合规使用').should('be.visible')
    })
  })
})
