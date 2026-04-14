/**
 * 部门管理 E2E 测试
 *
 * 覆盖维度：
 * - 正常路径：页面渲染、统计卡片、组织架构树、配额/权限卡片、成员表格
 * - 导航：URL 直接访问
 */

describe('部门管理', () => {
  beforeEach(() => {
    cy.loginByState()
  })

  describe('正常路径', () => {
    beforeEach(() => {
      cy.visit('/departments')
    })

    it('应渲染面包屑、标题和操作按钮', () => {
      cy.contains('系统').should('be.visible')
      cy.contains('部门管理').should('be.visible')
      cy.contains('组织架构、费用配额、知识库权限与模型白名单管理').should('be.visible')
      cy.contains('新建部门').should('be.visible')
    })

    it('应渲染统计卡片', () => {
      cy.contains('成员数').should('be.visible')
      cy.contains('每日消耗').should('be.visible')
      cy.contains('每日配额').should('be.visible')
      cy.contains('模型白名单').should('be.visible')
      cy.contains('技能白名单').should('be.visible')
    })

    it('应渲染组织架构树和配额/权限卡片', () => {
      cy.contains('组织架构').should('be.visible')
      cy.get('input[placeholder="搜索部门..."]').should('be.visible')
      cy.contains('全部状态').should('be.visible')
      cy.contains(/公司级配额设置|费用限额设置/).should('be.visible')
      cy.contains('权限与白名单配置').should('be.visible')
      cy.contains('可用模型白名单').should('be.visible')
      cy.contains('可用技能白名单').should('be.visible')
    })

    it('应渲染部门成员表格', () => {
      cy.contains('// 部门成员').should('be.visible')
      cy.contains('用户名').should('be.visible')
      cy.contains('邮箱').should('be.visible')
      cy.contains('加入时间').should('be.visible')
    })

    it('应可打开技能白名单弹窗', () => {
      cy.contains('可用技能白名单')
        .parents('div')
        .first()
        .within(() => {
          cy.contains('编辑').click()
        })

      cy.contains('编辑技能白名单').should('be.visible')
      cy.contains('保存白名单').should('be.visible')
    })
  })
})
