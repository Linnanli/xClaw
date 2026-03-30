/**
 * 扩展管理 E2E 测试
 *
 * 覆盖维度：
 * - 正常路径：页面渲染、Tab 栏、技能表格
 * - Tab 切换：技能/插件切换
 * - 导航：URL 直接访问
 */

describe('扩展管理', () => {
  beforeEach(() => {
    cy.loginByState()
  })

  describe('正常路径', () => {
    beforeEach(() => {
      cy.visit('/extensions')
    })

    it('应渲染页面标题', () => {
      cy.contains('扩展管理').should('be.visible')
      cy.contains('管理 AI 助手可用的技能和插件').should('be.visible')
    })

    it('应渲染 2 个 Tab', () => {
      cy.contains('技能管理').should('be.visible')
      cy.contains('插件管理').should('be.visible')
    })

    it('应渲染 4 行技能数据', () => {
      cy.contains('网页搜索').should('be.visible')
      cy.contains('代码执行').should('be.visible')
      cy.contains('文件分析').should('be.visible')
      cy.contains('图片生成').should('be.visible')
    })
  })

  describe('Tab 切换', () => {
    beforeEach(() => {
      cy.visit('/extensions')
    })

    it('点击插件管理 Tab 应显示插件数据', () => {
      cy.get('main').contains('button', '插件管理').click()
      cy.contains('Slack 集成').should('be.visible')
      cy.contains('飞书集成').should('be.visible')
      cy.contains('Jira 集成').should('be.visible')
      // 技能数据应隐藏
      cy.contains('网页搜索').should('not.exist')
    })

    it('切换回技能管理 Tab 应恢复原内容', () => {
      cy.get('main').contains('button', '插件管理').click()
      cy.contains('Slack 集成').should('be.visible')
      cy.get('main').contains('button', '技能管理').click()
      cy.contains('网页搜索').should('be.visible')
      cy.contains('Slack 集成').should('not.exist')
    })
  })

  describe('导航', () => {
    it('通过 URL 直接访问扩展管理', () => {
      cy.visit('/extensions')
      cy.contains('扩展管理').should('be.visible')
    })
  })
})
