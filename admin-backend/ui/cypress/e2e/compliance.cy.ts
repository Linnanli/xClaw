/**
 * 合规管理 E2E 测试
 *
 * 覆盖维度：
 * - 正常路径：页面渲染、分级卡片、报告表格
 * - 导航：侧边栏跳转
 */

describe('合规管理', () => {
  beforeEach(() => {
    cy.loginByState()
  })

  describe('正常路径', () => {
    beforeEach(() => {
      cy.visit('/compliance')
    })

    it('应渲染页面标题和操作按钮', () => {
      cy.contains('合规管理').should('be.visible')
      cy.contains('数据分类分级、合规报告与数据保留策略').should('be.visible')
      cy.contains('生成报告').should('be.visible')
    })

    it('应渲染 4 个数据分级卡片', () => {
      cy.contains('公开').should('be.visible')
      cy.contains('内部').should('be.visible')
      cy.contains('机密').should('be.visible')
      cy.contains('绝密').should('be.visible')
      cy.contains('892 次拦截').should('be.visible')
      cy.contains('2,156 次拦截').should('be.visible')
      cy.contains('847 次拦截').should('be.visible')
      cy.contains('1,284 次拦截').should('be.visible')
    })

    it('应渲染历史合规报告表格', () => {
      cy.contains('历史合规报告').should('be.visible')
      cy.contains('报告名称').should('be.visible')
      cy.contains('2024年6月合规报告').should('be.visible')
      cy.contains('2024年Q2合规报告').should('be.visible')
      cy.contains('2023年度合规报告').should('be.visible')
      cy.contains('月度报告').should('be.visible')
      cy.contains('下载 PDF').should('have.length.at.least', 1)
    })
  })

  describe('导航', () => {
    it('从侧边栏点击合规管理能跳转', () => {
      cy.visit('/')
      cy.get('aside').contains('合规管理').click()
      cy.url().should('include', '/compliance')
      cy.contains('数据分类分级、合规报告与数据保留策略').should('be.visible')
    })
  })
})
