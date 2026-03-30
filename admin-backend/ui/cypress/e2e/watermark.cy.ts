/**
 * 水印追踪 E2E 测试
 *
 * 覆盖维度：
 * - 正常路径：页面渲染、配置卡片、提取历史表格
 * - 导航：侧边栏跳转
 */

describe('水印追踪', () => {
  beforeEach(() => {
    cy.loginByState()
  })

  describe('正常路径', () => {
    beforeEach(() => {
      cy.visit('/watermark')
    })

    it('应渲染页面标题和操作按钮', () => {
      cy.contains('水印追踪').should('be.visible')
      cy.contains('为 AI 输出添加追踪水印，泄露时追溯来源').should('be.visible')
      cy.contains('提取水印').should('be.visible')
    })

    it('应渲染水印配置和预览卡片', () => {
      cy.contains('水印配置').should('be.visible')
      cy.contains('启用水印').should('be.visible')
      cy.contains('文本水印').should('be.visible')
      cy.contains('水印内容模板').should('be.visible')
      cy.contains('{{username}} - {{department}} - {{timestamp}}').should('be.visible')
      cy.contains('水印预览').should('be.visible')
      cy.get('[data-testid="watermark-preview"]').should('be.visible')
    })

    it('应渲染水印提取历史表格', () => {
      cy.contains('水印提取历史').should('be.visible')
      cy.contains('产品需求文档_v2.pdf').should('be.visible')
      cy.contains('zhang.wei').should('be.visible')
      cy.contains('提取成功').should('be.visible')
      cy.contains('提取失败').should('be.visible')
    })
  })

  describe('导航', () => {
    it('从侧边栏点击水印追踪能跳转', () => {
      cy.visit('/')
      cy.get('aside').contains('水印追踪').click()
      cy.url().should('include', '/watermark')
      cy.contains('为 AI 输出添加追踪水印，泄露时追溯来源').should('be.visible')
    })
  })
})
