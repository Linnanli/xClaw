/**
 * 统计报表 E2E 测试
 *
 * 覆盖维度：
 * - 正常路径：页面渲染、统计卡片、图表占位
 * - 导航：URL 直接访问
 */

describe('统计报表', () => {
  beforeEach(() => {
    cy.loginByState()
  })

  describe('正常路径', () => {
    beforeEach(() => {
      cy.visit('/reports')
    })

    it('应渲染页面标题和操作按钮', () => {
      cy.contains('统计报表').should('be.visible')
      cy.contains('DLP、安全事件、AI 使用量与费用统计').should('be.visible')
      cy.contains('导出 PDF').should('be.visible')
      cy.contains('导出 Excel').should('be.visible')
    })

    it('应渲染 4 个统计卡片', () => {
      cy.contains('DLP 规则数').should('be.visible')
      cy.contains('敏感操作数').should('be.visible')
      cy.contains('策略变更数').should('be.visible')
      cy.contains('近7天变更').should('be.visible')
      cy.get('main').contains('47').should('be.visible')
      cy.get('main').contains('12').should('be.visible')
      cy.get('main').contains('156').should('be.visible')
      cy.get('main').contains('23').should('be.visible')
    })

    it('应渲染 2 个图表占位区', () => {
      cy.contains('DLP 规则严重级别分布').should('be.visible')
      cy.contains('AI 使用量趋势').should('be.visible')
      cy.get('[data-testid="chart-dlp-severity"]').should('be.visible')
      cy.get('[data-testid="chart-ai-usage"]').should('be.visible')
    })
  })

  describe('导航', () => {
    it('通过 URL 直接访问统计报表', () => {
      cy.visit('/reports')
      cy.contains('统计报表').should('be.visible')
      cy.contains('DLP、安全事件、AI 使用量与费用统计').should('be.visible')
    })
  })
})
