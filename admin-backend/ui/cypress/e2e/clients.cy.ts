/**
 * 客户端管理 E2E 测试
 *
 * 覆盖维度：
 * - 正常路径：页面渲染、统计卡片、表格
 * - 导航：侧边栏跳转
 */

describe('客户端管理', () => {
  beforeEach(() => {
    cy.loginByState()
  })

  describe('正常路径', () => {
    beforeEach(() => {
      cy.visit('/clients')
    })

    it('应渲染页面标题和操作按钮', () => {
      cy.contains('客户端管理').should('be.visible')
      cy.contains('管理已连接的桌面客户端、设备指纹与策略推送').should('be.visible')
      cy.contains('批量推送').should('be.visible')
    })

    it('应渲染 4 个统计卡片', () => {
      cy.contains('总计').should('be.visible')
      cy.contains('在线').should('be.visible')
      cy.contains('离线').should('be.visible')
      cy.contains('需升级').should('be.visible')
      cy.contains('89').should('be.visible')
      cy.contains('47').should('be.visible')
      cy.contains('42').should('be.visible')
      cy.contains('12').should('be.visible')
    })

    it('应渲染筛选栏和客户端计数', () => {
      cy.contains('共 89 个客户端').should('be.visible')
      cy.contains('状态').should('be.visible')
      cy.contains('系统').should('be.visible')
    })

    it('应渲染表格表头', () => {
      cy.contains('客户端').should('be.visible')
      cy.contains('用户').should('be.visible')
      cy.contains('策略版本').should('be.visible')
      cy.contains('设备指纹').should('be.visible')
    })

    it('应渲染 3 行 Mock 数据', () => {
      cy.contains('DEV-WS-001').should('be.visible')
      cy.contains('DEV-WS-002').should('be.visible')
      cy.contains('MKT-WS-003').should('be.visible')
      cy.contains('已验证').should('be.visible')
      cy.contains('未验证').should('be.visible')
      cy.contains('需升级').should('be.visible')
    })
  })

  describe('导航', () => {
    it('从侧边栏点击客户端能跳转', () => {
      cy.visit('/')
      cy.get('aside').contains('客户端').click()
      cy.url().should('include', '/clients')
      cy.contains('管理已连接的桌面客户端、设备指纹与策略推送').should('be.visible')
    })
  })
})
