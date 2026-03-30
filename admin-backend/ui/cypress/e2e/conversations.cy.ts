/**
 * 对话审计 E2E 测试
 *
 * 覆盖维度：
 * - 正常路径：页面渲染、统计卡片、表格
 * - 导航：侧边栏跳转
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
      cy.get('main').contains('342').should('be.visible')
      cy.get('main').contains('1.2M').should('be.visible')
      cy.get('main').contains('18').should('be.visible')
      cy.get('main').contains('67').should('be.visible')
    })

    it('应渲染筛选栏和对话计数', () => {
      cy.contains('共 1,284 条对话').should('be.visible')
      cy.contains('DLP').should('be.visible')
    })

    it('应渲染表格表头和 4 行 Mock 数据', () => {
      cy.contains('用户').should('be.visible')
      cy.contains('对话主题').should('be.visible')
      cy.contains('消息数').should('be.visible')
      cy.contains('TOKEN').should('be.visible')
      cy.contains('产品需求分析').should('be.visible')
      cy.contains('代码审查辅助').should('be.visible')
      cy.contains('HR 政策咨询').should('be.visible')
      cy.contains('市场报告生成').should('be.visible')
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
