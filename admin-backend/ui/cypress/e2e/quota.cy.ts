/**
 * 配额管理 E2E 测试
 *
 * 覆盖维度：
 * - 正常路径：页面渲染、统计卡片、预算进度条、排行区
 * - 导航：侧边栏跳转
 */

describe('配额管理', () => {
  beforeEach(() => {
    cy.loginByState()
  })

  describe('正常路径', () => {
    beforeEach(() => {
      cy.visit('/quota')
    })

    it('应渲染页面标题和操作按钮', () => {
      cy.contains('配额管理').should('be.visible')
      cy.contains('Token 使用量、费用统计与配额控制').should('be.visible')
      cy.contains('配额设置').should('be.visible')
    })

    it('应渲染 4 个统计卡片', () => {
      cy.contains('今日消耗').should('be.visible')
      cy.contains('本月消耗').should('be.visible')
      cy.contains('本月预算').should('be.visible')
      cy.contains('活跃模型').should('be.visible')
      cy.contains('2.4M').should('be.visible')
      cy.contains('48.7M').should('be.visible')
      cy.contains('¥2,400').should('be.visible')
    })

    it('应渲染预算进度条', () => {
      cy.contains('月度预算使用率').should('be.visible')
      cy.contains('¥1,632 / ¥2,400').should('be.visible')
      cy.contains('68% 已使用').should('be.visible')
      cy.get('[data-testid="budget-progress"]').should('be.visible')
    })

    it('应渲染部门排行和模型排行', () => {
      cy.contains('部门 Token 消耗').should('be.visible')
      cy.contains('研发部').should('be.visible')
      cy.contains('18.2M').should('be.visible')
      cy.contains('产品部').should('be.visible')
      cy.contains('市场部').should('be.visible')
      cy.contains('运营部').should('be.visible')

      cy.contains('模型调用量').should('be.visible')
      cy.contains('deepseek-chat').should('be.visible')
      cy.contains('qwen-plus').should('be.visible')
      cy.contains('moonshot-v1').should('be.visible')
      cy.contains('ollama-local').should('be.visible')
      cy.contains('¥842').should('be.visible')
    })
  })

  describe('导航', () => {
    it('从侧边栏点击配额管理能跳转', () => {
      cy.visit('/')
      cy.get('aside').contains('配额管理').click()
      cy.url().should('include', '/quota')
      cy.contains('Token 使用量、费用统计与配额控制').should('be.visible')
    })
  })
})
