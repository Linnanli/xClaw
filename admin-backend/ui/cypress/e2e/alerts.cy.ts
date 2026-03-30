/**
 * 告警中心 E2E 测试
 *
 * 覆盖维度：
 * - 正常路径：页面渲染、统计卡片、表格
 * - 导航：侧边栏跳转
 */

describe('告警中心', () => {
  beforeEach(() => {
    cy.loginByState()
  })

  describe('正常路径', () => {
    beforeEach(() => {
      cy.visit('/alerts')
    })

    it('应渲染页面标题和操作按钮', () => {
      cy.contains('告警中心').should('be.visible')
      cy.contains('安全事件检测、告警规则管理与多渠道通知').should('be.visible')
      cy.contains('告警规则').should('be.visible')
      cy.contains('新建规则').should('be.visible')
    })

    it('应渲染 4 个统计卡片', () => {
      cy.contains('未处理').should('be.visible')
      cy.contains('处理中').should('be.visible')
      cy.contains('今日告警').should('be.visible')
      cy.contains('已关闭').should('be.visible')
      cy.get('main').contains('7').should('be.visible')
      cy.get('main').contains('3').should('be.visible')
      cy.get('main').contains('23').should('be.visible')
      cy.get('main').contains('156').should('be.visible')
    })

    it('应渲染筛选栏和告警计数', () => {
      cy.contains('共 189 条告警').should('be.visible')
      cy.contains('级别').should('be.visible')
      cy.contains('状态').should('be.visible')
    })

    it('应渲染表格表头和 4 行 Mock 数据', () => {
      cy.contains('时间').should('be.visible')
      cy.contains('规则名称').should('be.visible')
      cy.contains('触发详情').should('be.visible')
      cy.contains('DLP 拦截阈值超限').should('be.visible')
      cy.contains('异常登录检测').should('be.visible')
      cy.contains('Token 配额预警').should('be.visible')
      cy.contains('模型服务异常').should('be.visible')
    })
  })

  describe('导航', () => {
    it('从侧边栏点击告警中心能跳转', () => {
      cy.visit('/')
      cy.get('aside').contains('告警中心').click()
      cy.url().should('include', '/alerts')
      cy.contains('安全事件检测、告警规则管理与多渠道通知').should('be.visible')
    })
  })
})
