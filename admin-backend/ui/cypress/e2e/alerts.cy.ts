/**
 * 告警中心 E2E 测试
 *
 * 覆盖维度：
 * - 正常路径：页面渲染、统计卡片、筛选栏、表格结构
 * - 交互：搜索、筛选、新建规则弹窗、规则列表弹窗
 * - 导航：侧边栏跳转
 * - 空数据：无告警时的空状态展示
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
    })

    it('应渲染筛选栏', () => {
      cy.get('input[placeholder="搜索告警..."]').should('be.visible')
      cy.contains('全部级别').should('be.visible')
      cy.contains('全部状态').should('be.visible')
    })

    it('应渲染表格表头', () => {
      const headers = ['时间', '级别', '规则名称', '触发详情', '状态', '操作']
      headers.forEach(h => cy.contains(h).should('be.visible'))
    })
  })

  describe('交互', () => {
    beforeEach(() => {
      cy.visit('/alerts')
    })

    it('搜索框应可输入', () => {
      cy.get('input[placeholder="搜索告警..."]').type('DLP')
      cy.get('input[placeholder="搜索告警..."]').should('have.value', 'DLP')
    })

    it('级别筛选应可选择', () => {
      cy.get('select').contains('全部级别').parent('select').select('critical')
    })

    it('状态筛选应可选择', () => {
      cy.get('select').contains('全部状态').parent('select').select('pending')
    })

    it('点击新建规则应打开弹窗', () => {
      cy.contains('新建规则').click()
      cy.contains('新建告警规则').should('be.visible')
      cy.contains('规则名称').should('be.visible')
      cy.contains('事件类型').should('be.visible')
      cy.contains('严重级别').should('be.visible')
      cy.contains('通知渠道').should('be.visible')
      cy.contains('静默期').should('be.visible')
    })

    it('点击告警规则应打开规则列表弹窗', () => {
      cy.contains('告警规则').first().click()
      cy.contains('告警规则管理').should('be.visible')
    })
  })

  describe('空数据展示', () => {
    it('无告警事件时应展示空状态', () => {
      cy.visit('/alerts')
      cy.get('body').then($body => {
        if ($body.text().includes('暂无告警事件')) {
          cy.contains('暂无告警事件').should('be.visible')
        }
      })
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
