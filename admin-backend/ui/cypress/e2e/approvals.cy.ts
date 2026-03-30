/**
 * 审批工单 E2E 测试
 *
 * 覆盖维度：
 * - 正常路径：页面渲染、统计卡片、表格
 * - 导航：侧边栏跳转
 */

describe('审批工单', () => {
  beforeEach(() => {
    cy.loginByState()
  })

  describe('正常路径', () => {
    beforeEach(() => {
      cy.visit('/approvals')
    })

    it('应渲染页面标题和操作按钮', () => {
      cy.contains('审批工单').should('be.visible')
      cy.contains('高风险操作审批流程管理').should('be.visible')
      cy.contains('审批规则配置').should('be.visible')
    })

    it('应渲染 4 个统计卡片', () => {
      cy.contains('待审批').should('be.visible')
      cy.contains('已批准').should('be.visible')
      cy.contains('已拒绝').should('be.visible')
      cy.contains('已过期').should('be.visible')
      cy.get('main').contains('5').should('be.visible')
      cy.get('main').contains('42').should('be.visible')
      cy.get('main').contains('8').should('be.visible')
    })

    it('应渲染表格表头和 4 行 Mock 数据', () => {
      cy.contains('申请人').should('be.visible')
      cy.contains('操作类型').should('be.visible')
      cy.contains('申请时间').should('be.visible')
      cy.contains('过期时间').should('be.visible')
      cy.contains('导出客户数据 (CSV)').should('be.visible')
      cy.contains('批量删除 DLP 规则 (5条)').should('be.visible')
      cy.contains('修改系统安全配置').should('be.visible')
      cy.contains('访问绝密级知识库').should('be.visible')
    })

    it('应渲染批准和拒绝按钮', () => {
      cy.contains('批准').should('be.visible')
      cy.contains('拒绝').should('be.visible')
      cy.contains('查看').should('be.visible')
    })
  })

  describe('导航', () => {
    it('从侧边栏点击审批工单能跳转', () => {
      cy.visit('/')
      cy.get('aside').contains('审批工单').click()
      cy.url().should('include', '/approvals')
      cy.contains('高风险操作审批流程管理').should('be.visible')
    })
  })
})
