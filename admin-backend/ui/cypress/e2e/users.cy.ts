/**
 * 用户管理 E2E 测试
 *
 * 覆盖维度：
 * - 正常路径：页面渲染、Tab、搜索、表格
 * - 弹窗交互：新建用户弹窗打开/关闭/验证
 * - Tab 切换：用户列表/角色管理/权限管理
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

    it('应渲染 3 个 Tab', () => {
      cy.contains('用户列表').should('be.visible')
      cy.contains('角色管理').should('be.visible')
      cy.contains('权限管理').should('be.visible')
    })

    it('应渲染搜索框', () => {
      cy.get('input[placeholder*="搜索用户"]').should('be.visible')
    })

    it('应渲染表格表头', () => {
      cy.contains('用户名').should('be.visible')
      cy.contains('邮箱').should('be.visible')
      cy.contains('创建时间').should('be.visible')
    })

    it('应渲染分页', () => {
      cy.get('[data-testid="table-pagination"]').should('be.visible')
    })
  })

  describe('新建用户弹窗', () => {
    beforeEach(() => {
      cy.visit('/users')
    })

    it('点击新建用户应打开弹窗', () => {
      cy.contains('新建用户').click()
      cy.contains('新建用户').should('be.visible')
    })

    it('弹窗应包含所有表单字段', () => {
      cy.contains('button', '新建用户').click()
      cy.get('input[placeholder="请输入用户名"]').should('be.visible')
      cy.get('input[placeholder="请输入邮箱"]').should('be.visible')
      cy.get('input[placeholder="请输入密码"]').should('be.visible')
      cy.get('input[placeholder="请再次输入密码"]').should('be.visible')
    })

    it('点击取消应关闭弹窗', () => {
      cy.contains('button', '新建用户').click()
      cy.get('input[placeholder="请输入用户名"]').should('be.visible')
      cy.contains('button', '取消').click()
      cy.get('input[placeholder="请输入用户名"]').should('not.exist')
    })

    it('不填字段点击创建应显示错误', () => {
      cy.contains('button', '新建用户').click()
      cy.contains('button', '创建').click()
      cy.contains('请填写所有必填字段').should('be.visible')
    })
  })

  describe('Tab 切换', () => {
    beforeEach(() => {
      cy.visit('/users')
    })

    it('点击角色管理 Tab 应切换内容', () => {
      cy.get('main').contains('button', '角色管理').click()
      // 用户列表搜索框应消失
      cy.get('input[placeholder*="搜索用户"]').should('not.exist')
    })

    it('点击权限管理 Tab 应切换内容', () => {
      cy.get('main').contains('button', '权限管理').click()
      cy.get('input[placeholder*="搜索用户"]').should('not.exist')
    })

    it('切换回用户列表 Tab 应恢复', () => {
      cy.get('main').contains('button', '角色管理').click()
      cy.get('main').contains('button', '用户列表').click()
      cy.get('input[placeholder*="搜索用户"]').should('be.visible')
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
