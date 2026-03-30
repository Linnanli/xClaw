/**
 * 安全策略 E2E 测试
 *
 * 覆盖维度：
 * - 正常路径：页面渲染、Tab、双向扫描横幅、搜索、表格
 * - Tab 切换：5 个 Tab 内容切换
 * - 导航：侧边栏跳转
 */

describe('安全策略', () => {
  beforeEach(() => {
    cy.loginByState()
  })

  describe('正常路径', () => {
    beforeEach(() => {
      cy.visit('/security')
    })

    it('应渲染页面标题', () => {
      cy.contains('安全策略').should('be.visible')
      cy.contains('DLP 规则、敏感词典、敏感操作与策略版本管理').should('be.visible')
    })

    it('应渲染 5 个 Tab', () => {
      cy.get('main').contains('DLP 规则').should('be.visible')
      cy.get('main').contains('敏感词典').should('be.visible')
      cy.get('main').contains('敏感操作').should('be.visible')
      cy.get('main').contains('策略版本').should('be.visible')
      cy.get('main').contains('拦截记录').should('be.visible')
    })

    it('应渲染双向扫描横幅', () => {
      cy.contains('双向扫描已启用').should('be.visible')
    })

    it('应渲染搜索框和筛选', () => {
      cy.get('input[placeholder*="搜索规则"]').should('be.visible')
      cy.contains('严重级别').should('be.visible')
      cy.contains('分类').should('be.visible')
    })

    it('应渲染表格表头', () => {
      cy.contains('规则名称').should('be.visible')
      cy.contains('严重级别').should('be.visible')
    })

    it('应渲染分页', () => {
      cy.get('[data-testid="table-pagination"]').should('be.visible')
    })
  })

  describe('Tab 切换', () => {
    beforeEach(() => {
      cy.visit('/security')
    })

    it('点击敏感词典 Tab 应切换内容', () => {
      cy.get('main').contains('button', '敏感词典').click()
      cy.contains('字典名称').should('be.visible')
      cy.contains('双向扫描已启用').should('not.exist')
    })

    it('点击敏感操作 Tab 应切换内容', () => {
      cy.get('main').contains('button', '敏感操作').click()
      cy.contains('操作名称').should('be.visible')
      cy.contains('风险等级').should('be.visible')
    })

    it('点击策略版本 Tab 应切换内容', () => {
      cy.get('main').contains('button', '策略版本').click()
      cy.contains('变更类型').should('be.visible')
    })

    it('点击拦截记录 Tab 应显示开发中提示', () => {
      cy.get('main').contains('button', '拦截记录').click()
      cy.contains('拦截记录功能开发中').should('be.visible')
    })

    it('切换回 DLP 规则 Tab 应恢复', () => {
      cy.get('main').contains('button', '敏感词典').click()
      cy.get('main').contains('button', 'DLP 规则').click()
      cy.contains('双向扫描已启用').should('be.visible')
    })
  })

  describe('导航', () => {
    it('从侧边栏点击安全策略能跳转', () => {
      cy.visit('/')
      cy.get('aside').contains('安全策略').click()
      cy.url().should('include', '/security')
    })
  })
})
