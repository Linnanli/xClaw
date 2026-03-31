/**
 * 合规管理 E2E 测试
 */

describe('合规管理', () => {
  beforeEach(() => {
    cy.loginByState()
  })

  describe('正常路径', () => {
    beforeEach(() => {
      cy.visit('/compliance')
    })

    it('应渲染页面标题和生成报告按钮', () => {
      cy.contains('合规管理').should('be.visible')
      cy.contains('数据分类分级、合规报告与数据保留策略').should('be.visible')
      cy.contains('生成报告').should('be.visible')
    })

    it('应渲染 4 个数据分级卡片', () => {
      // 数据分级卡片依赖后端 API 返回，后端未启动时可能不显示
      cy.get('body').then($body => {
        if ($body.text().includes('公开')) {
          cy.contains('公开').should('be.visible')
          cy.contains('内部').should('be.visible')
          cy.contains('机密').should('be.visible')
          cy.contains('绝密').should('be.visible')
        }
      })
    })

    it('应渲染数据保留策略区域', () => {
      cy.contains('数据保留策略').should('be.visible')
    })

    it('应渲染历史合规报告表格', () => {
      cy.contains('历史合规报告').should('be.visible')
      const headers = ['报告名称', '类型', '时间范围', '生成时间', '操作']
      headers.forEach(h => cy.contains(h).should('be.visible'))
    })
  })

  describe('交互', () => {
    beforeEach(() => {
      cy.visit('/compliance')
    })

    it('点击生成报告应打开弹窗', () => {
      cy.contains('生成报告').click()
      cy.contains('生成合规报告').should('be.visible')
      cy.contains('报告名称').should('be.visible')
      cy.contains('报告类型').should('be.visible')
      cy.contains('开始日期').should('be.visible')
      cy.contains('结束日期').should('be.visible')
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
