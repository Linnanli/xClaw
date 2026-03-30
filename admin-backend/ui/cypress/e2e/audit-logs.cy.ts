/**
 * 审计日志 E2E 测试
 *
 * 覆盖维度：
 * - 正常路径：页面渲染、筛选栏、日志表格、分页
 * - 导航：URL 直接访问
 */

describe('审计日志', () => {
  beforeEach(() => {
    cy.loginByState()
  })

  describe('正常路径', () => {
    beforeEach(() => {
      cy.visit('/audit-logs')
    })

    it('应渲染页面标题', () => {
      cy.contains('审计日志').should('be.visible')
      cy.contains('查看和导出所有管理操作的审计记录').should('be.visible')
    })

    it('应渲染筛选栏和导出按钮', () => {
      cy.contains('操作类型').should('be.visible')
      cy.contains('日期范围').should('be.visible')
      cy.contains('导出 CSV').should('be.visible')
    })

    it('应渲染 5 行日志数据', () => {
      cy.contains('创建用户').should('be.visible')
      cy.contains('修改规则').should('be.visible')
      cy.contains('登录系统').should('be.visible')
      cy.contains('删除规则').should('be.visible')
      cy.contains('导出数据').should('be.visible')
      cy.contains('192.168.1.100').should('have.length.at.least', 1)
    })

    it('应渲染分页和记录总数', () => {
      cy.get('[data-testid="table-pagination"]').should('be.visible')
      cy.get('[data-testid="table-pagination"]').contains('共 1,284 条记录').should('be.visible')
    })
  })
})
