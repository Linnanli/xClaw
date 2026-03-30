/**
 * 系统设置 E2E 测试
 *
 * 覆盖维度：
 * - 正常路径：页面渲染、Tab 栏、DLP/安全/通知配置
 * - 导航：侧边栏跳转
 */

describe('系统设置', () => {
  beforeEach(() => {
    cy.loginByState()
  })

  describe('正常路径', () => {
    beforeEach(() => {
      cy.visit('/settings')
    })

    it('应渲染页面标题和操作按钮', () => {
      cy.contains('系统设置').should('be.visible')
      cy.contains('DLP、审计、客户端、告警通知、安全策略与水印配置').should('be.visible')
      cy.contains('恢复默认').should('be.visible')
      cy.contains('保存配置').should('be.visible')
    })

    it('应渲染 7 个 Tab 及新增 badge', () => {
      cy.contains('DLP 防泄漏').should('be.visible')
      cy.contains('审计日志').should('be.visible')
      cy.contains('客户端').should('be.visible')
      cy.contains('策略同步').should('be.visible')
      cy.contains('告警通知').should('be.visible')
      cy.contains('安全策略').should('be.visible')
      cy.contains('水印配置').should('be.visible')
      cy.get('main').contains('新增').should('have.length.at.least', 1)
    })

    it('应渲染 DLP、安全策略和告警通知配置', () => {
      cy.contains('DLP 数据防泄漏').should('be.visible')
      cy.contains('启用 DLP 扫描').should('be.visible')
      cy.contains('双向扫描').should('be.visible')
      cy.contains('5000ms').should('be.visible')
      cy.contains('故障开放模式').should('be.visible')
      cy.contains('安全策略').should('be.visible')
      cy.contains('8 位').should('be.visible')
      cy.contains('480 分钟').should('be.visible')
      cy.contains('5 次 / 15 分钟').should('be.visible')
      cy.contains('告警通知渠道').should('be.visible')
      cy.contains('邮件').should('be.visible')
      cy.contains('企微').should('be.visible')
      cy.contains('钉钉').should('be.visible')
      cy.contains('飞书').should('be.visible')
    })
  })

  describe('导航', () => {
    it('从侧边栏点击系统设置能跳转', () => {
      cy.visit('/')
      cy.get('aside').contains('系统设置').click()
      cy.url().should('include', '/settings')
      cy.contains('DLP、审计、客户端、告警通知、安全策略与水印配置').should('be.visible')
    })
  })
})
