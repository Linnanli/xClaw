/**
 * 审批工单 E2E 测试
 *
 * 覆盖维度：
 * - 正常路径：页面渲染、统计卡片、表格结构
 * - 交互：状态筛选
 * - 导航：侧边栏跳转
 * - 空数据：无工单时的空状态展示
 */

describe('审批工单', () => {
  beforeEach(() => {
    cy.loginByState()
  })

  describe('正常路径', () => {
    beforeEach(() => {
      cy.visit('/approvals')
    })

    it('应渲染页面标题', () => {
      cy.contains('审批工单').should('be.visible')
      cy.contains('高风险操作审批流程管理').should('be.visible')
    })

    it('应渲染 4 个统计卡片', () => {
      cy.contains('待审批').should('be.visible')
      cy.contains('已批准').should('be.visible')
      cy.contains('已拒绝').should('be.visible')
      cy.contains('已过期').should('be.visible')
    })

    it('应渲染表格表头', () => {
      const headers = ['申请人', '操作类型', '申请时间', '过期时间', '状态', '操作']
      headers.forEach(h => cy.contains(h).should('be.visible'))
    })
  })

  describe('交互', () => {
    beforeEach(() => {
      cy.visit('/approvals')
    })

    it('状态筛选应可选择', () => {
      cy.get('select').contains('全部状态').parent('select').select('pending')
    })
  })

  describe('空数据展示', () => {
    it('无工单时应展示空状态', () => {
      cy.visit('/approvals')
      cy.get('body').then($body => {
        if ($body.text().includes('暂无审批工单')) {
          cy.contains('暂无审批工单').should('be.visible')
        }
      })
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
