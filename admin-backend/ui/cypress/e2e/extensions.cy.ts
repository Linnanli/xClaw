/**
 * 扩展管理 E2E 测试
 *
 * 覆盖维度：
 * - 正常路径：页面渲染、Tab 栏、技能表格
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
      cy.contains('v1.2.0').should('be.visible')
      cy.contains('IronClaw').should('have.length.at.least', 1)
      cy.contains('Community').should('be.visible')
    })
  })

  describe('导航', () => {
    it('通过 URL 直接访问扩展管理', () => {
      cy.visit('/extensions')
      cy.contains('扩展管理').should('be.visible')
      cy.contains('管理 AI 助手可用的技能和插件').should('be.visible')
    })
  })
})
