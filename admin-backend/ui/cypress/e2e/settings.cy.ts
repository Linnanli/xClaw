/**
 * 系统设置 E2E 测试
 *
 * 覆盖维度：
 * - 正常路径：页面渲染、Tab 栏、DLP 配置
 * - Tab 切换：7 个 Tab 内容切换
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

    it('应渲染 DLP 防泄漏默认内容', () => {
      cy.contains('DLP 数据防泄漏').should('be.visible')
      cy.contains('启用 DLP 扫描').should('be.visible')
      cy.contains('双向扫描').should('be.visible')
      cy.contains('5000ms').should('be.visible')
      cy.contains('故障开放模式').should('be.visible')
    })
  })

  describe('Tab 切换', () => {
    beforeEach(() => {
      cy.visit('/settings')
    })

    it('点击审计日志 Tab 应显示审计配置', () => {
      cy.get('main').contains('button', '审计日志').click()
      cy.contains('启用审计').should('be.visible')
      cy.contains('90 天').should('be.visible')
      // DLP 内容应隐藏
      cy.contains('DLP 数据防泄漏').should('not.exist')
    })

    it('点击客户端 Tab 应显示客户端配置', () => {
      cy.get('main').contains('button', '客户端').click()
      cy.contains('客户端配置').should('be.visible')
      cy.contains('自动更新').should('be.visible')
      cy.contains('v2.1.0').should('be.visible')
    })

    it('点击告警通知 Tab 应显示通知渠道', () => {
      cy.get('main').contains('button', '告警通知').click()
      cy.contains('告警通知渠道').should('be.visible')
      cy.contains('邮件').should('be.visible')
      cy.contains('企微').should('be.visible')
      cy.contains('钉钉').should('be.visible')
      cy.contains('飞书').should('be.visible')
    })

    it('点击安全策略 Tab 应显示安全配置', () => {
      cy.get('main').contains('button', '安全策略').click()
      cy.contains('密码最小长度').should('be.visible')
      cy.contains('8 位').should('be.visible')
      cy.contains('480 分钟').should('be.visible')
    })

    it('点击水印配置 Tab 应显示水印设置', () => {
      cy.get('main').contains('button', '水印配置').click()
      cy.contains('启用水印').should('be.visible')
      cy.contains('文本水印').should('be.visible')
      cy.contains('15%').should('be.visible')
    })

    it('切换回 DLP 防泄漏 Tab 应恢复原内容', () => {
      cy.get('main').contains('button', '审计日志').click()
      cy.contains('启用审计').should('be.visible')
      cy.get('main').contains('button', 'DLP 防泄漏').click()
      cy.contains('DLP 数据防泄漏').should('be.visible')
      cy.contains('5000ms').should('be.visible')
    })
  })

  describe('导航', () => {
    it('从侧边栏点击系统设置能跳转', () => {
      cy.visit('/')
      cy.get('aside').contains('系统设置').click()
      cy.url().should('include', '/settings')
    })
  })
})
