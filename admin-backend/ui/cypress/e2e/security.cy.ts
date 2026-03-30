/**
 * 安全策略 E2E 测试
 *
 * 覆盖维度：
 * - 正常路径：页面渲染、Tab、双向扫描横幅、表格
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

    it('应渲染页面标题和操作按钮', () => {
      cy.contains('安全策略').should('be.visible')
      cy.contains('DLP 规则、敏感词典、敏感操作与策略版本管理').should('be.visible')
      cy.contains('导入').should('be.visible')
      cy.contains('新建规则').should('be.visible')
    })

    it('应渲染 5 个 Tab 且 DLP 规则为活跃状态', () => {
      // 验证 5 个 Tab 都存在
      cy.get('main').contains('DLP 规则').should('be.visible')
      cy.get('main').contains('敏感词典').should('be.visible')
      cy.get('main').contains('敏感操作').should('be.visible')
      cy.get('main').contains('策略版本').should('be.visible')
      cy.get('main').contains('拦截记录').should('be.visible')
      // 活跃 Tab 应有绿色下边框
      cy.get('main').contains('button', 'DLP 规则')
        .should('have.css', 'border-bottom-style', 'solid')
    })

    it('应渲染双向扫描横幅', () => {
      cy.contains('双向扫描已启用').should('be.visible')
      cy.contains('输入和输出均受 DLP 引擎保护').should('be.visible')
      cy.contains('新增').should('be.visible')
    })

    it('应渲染筛选栏和规则计数', () => {
      cy.contains('共 24 条规则').should('be.visible')
      cy.contains('严重级别').should('be.visible')
      cy.contains('数据分级').should('be.visible')
    })

    it('应渲染表格表头', () => {
      cy.contains('规则名称').should('be.visible')
      cy.contains('级别').should('be.visible')
      cy.contains('扫描方向').should('be.visible')
      cy.contains('命中数').should('be.visible')
    })

    it('应渲染 4 行 Mock 数据', () => {
      cy.contains('身份证号码检测').should('be.visible')
      cy.contains('银行卡号检测').should('be.visible')
      cy.contains('手机号码检测').should('be.visible')
      cy.contains('邮箱地址检测').should('be.visible')
      cy.contains('1,284').should('be.visible')
      cy.contains('关闭').should('be.visible')
    })
  })

  describe('导航', () => {
    it('从侧边栏点击安全策略能跳转', () => {
      cy.visit('/')
      cy.get('aside').contains('安全策略').click()
      cy.url().should('include', '/security')
      cy.contains('DLP 规则、敏感词典、敏感操作与策略版本管理').should('be.visible')
    })
  })
})
