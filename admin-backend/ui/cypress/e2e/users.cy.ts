/**
 * 用户管理 E2E 测试
 *
 * 覆盖维度：
 * - 正常路径：页面渲染、Tab、表格、筛选栏
 * - 导航：侧边栏跳转
 */

describe('用户管理', () => {
  beforeEach(() => {
    cy.loginByState()
  })

  describe('正常路径', () => {
    beforeEach(() => {
      cy.visit('/users')
    })

    it('应渲染页面标题和操作按钮', () => {
      cy.contains('用户管理').should('be.visible')
      cy.contains('用户账户、角色与权限管理').should('be.visible')
      cy.contains('批量导入').should('be.visible')
      cy.contains('LDAP 同步').should('be.visible')
      cy.contains('新建用户').should('be.visible')
    })

    it('应渲染 3 个 Tab 且用户列表为活跃状态', () => {
      cy.contains('用户列表').should('be.visible')
      cy.contains('角色管理').should('be.visible')
      cy.contains('权限管理').should('be.visible')
      // 活跃 Tab 应有绿色文字
      cy.contains('用户列表').should('have.css', 'color', 'rgb(10, 107, 58)')
    })

    it('应渲染筛选栏和用户计数', () => {
      cy.contains('共 89 个用户').should('be.visible')
      cy.contains('角色').should('be.visible')
      cy.contains('部门').should('be.visible')
    })

    it('应渲染表格表头', () => {
      cy.contains('用户名').should('be.visible')
      cy.contains('邮箱').should('be.visible')
      cy.contains('角色').should('be.visible')
      cy.contains('MFA').should('be.visible')
      cy.contains('状态').should('be.visible')
      cy.contains('操作').should('be.visible')
    })

    it('应渲染 4 行 Mock 数据', () => {
      cy.contains('zhang.wei').should('be.visible')
      cy.contains('li.ming').should('be.visible')
      cy.contains('wang.fang').should('be.visible')
      cy.contains('chen.jing').should('be.visible')
      cy.contains('超级管理员').should('be.visible')
      cy.contains('禁用').should('be.visible')
    })
  })

  describe('导航', () => {
    it('从侧边栏点击用户管理能跳转', () => {
      cy.visit('/')
      cy.get('aside').contains('用户管理').click()
      cy.url().should('include', '/users')
      cy.contains('用户账户、角色与权限管理').should('be.visible')
    })
  })
})
