/**
 * 部门管理 E2E 测试
 *
 * 覆盖维度：
 * - 正常路径：页面渲染、面包屑、统计卡片、组织架构树、配额/权限卡片、成员表格
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
      cy.contains('组织架构、Token 配额、知识库权限与模型白名单管理').should('be.visible')
      cy.contains('新建部门').should('be.visible')
    })

    it('应渲染 4 个统计卡片', () => {
      cy.contains('成员数').should('be.visible')
      cy.contains('月 Token 消耗').should('be.visible')
      cy.contains('月配额').should('be.visible')
      cy.contains('知识库').should('be.visible')
      cy.get('main').contains('89').should('be.visible')
      cy.get('main').contains('48.7M').should('be.visible')
      cy.get('main').contains('100M').should('be.visible')
    })

    it('应渲染组织架构树和配额/权限卡片', () => {
      cy.contains('组织架构').should('be.visible')
      cy.get('[data-testid="org-tree"]').should('be.visible')
      cy.contains('IronClaw 总部').should('be.visible')
      cy.contains('研发部').should('be.visible')
      cy.contains('前端组').should('be.visible')
      cy.contains('后端组').should('be.visible')
      cy.contains('Token 配额设置').should('be.visible')
      cy.contains('5,000,000').should('be.visible')
      cy.contains('100,000,000').should('be.visible')
      cy.contains('权限与白名单配置').should('be.visible')
      cy.contains('可用知识库').should('be.visible')
      cy.contains('可用模型白名单').should('be.visible')
    })

    it('应渲染部门成员表格', () => {
      cy.contains('// 部门成员').should('be.visible')
      cy.contains('研发部 — 部门配置').should('be.visible')
      cy.contains('zhang.wei').should('be.visible')
      cy.contains('li.ming').should('be.visible')
      cy.contains('管理员').should('be.visible')
    })
  })
})
