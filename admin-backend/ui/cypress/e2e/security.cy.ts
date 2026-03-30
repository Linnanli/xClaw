/**
 * 安全策略 E2E 测试
 *
 * 覆盖维度：
 * - 正常路径：页面渲染、Tab、双向扫描横幅、表格
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

    it('应渲染页面标题和操作按钮', () => {
      cy.contains('安全策略').should('be.visible')
      cy.contains('DLP 规则、敏感词典、敏感操作与策略版本管理').should('be.visible')
      cy.contains('导入').should('be.visible')
      cy.contains('新建规则').should('be.visible')
    })

    it('应渲染 5 个 Tab 且 DLP 规则为活跃状态', () => {
      cy.get('main').contains('DLP 规则').should('be.visible')
      cy.get('main').contains('敏感词典').should('be.visible')
      cy.get('main').contains('敏感操作').should('be.visible')
      cy.get('main').contains('策略版本').should('be.visible')
      cy.get('main').contains('拦截记录').should('be.visible')
      cy.get('main').contains('button', 'DLP 规则')
        .should('have.css', 'border-bottom-style', 'solid')
    })

    it('应渲染双向扫描横幅', () => {
      cy.contains('双向扫描已启用').should('be.visible')
      cy.contains('输入和输出均受 DLP 引擎保护').should('be.visible')
    })

    it('应渲染筛选栏和规则计数', () => {
      cy.contains('共 24 条规则').should('be.visible')
      cy.contains('严重级别').should('be.visible')
      cy.contains('数据分级').should('be.visible')
    })

    it('应渲染表格表头', () => {
      cy.contains('规则名称').should('be.visible')
      cy.contains('扫描方向').should('be.visible')
      cy.contains('命中数').should('be.visible')
    })

    it('应渲染 4 行 Mock 数据', () => {
      cy.contains('身份证号码检测').should('be.visible')
      cy.contains('银行卡号检测').should('be.visible')
      cy.contains('手机号码检测').should('be.visible')
      cy.contains('邮箱地址检测').should('be.visible')
    })
  })

  describe('Tab 切换', () => {
    beforeEach(() => {
      cy.visit('/security')
    })

    it('点击敏感词典 Tab 应显示词典表格', () => {
      cy.get('main').contains('button', '敏感词典').click()
      cy.contains('身份证号码库').should('be.visible')
      cy.contains('银行卡号库').should('be.visible')
      cy.contains('手机号码库').should('be.visible')
      // DLP 规则内容应隐藏
      cy.contains('双向扫描已启用').should('not.exist')
    })

    it('点击敏感操作 Tab 应显示操作表格', () => {
      cy.get('main').contains('button', '敏感操作').click()
      cy.contains('批量导出数据').should('be.visible')
      cy.contains('删除对话记录').should('be.visible')
      cy.contains('修改 DLP 规则').should('be.visible')
    })

    it('点击策略版本 Tab 应显示版本历史', () => {
      cy.get('main').contains('button', '策略版本').click()
      cy.contains('v2.4.0').should('be.visible')
      cy.contains('新增双向扫描支持').should('be.visible')
      cy.contains('当前版本').should('be.visible')
      cy.contains('历史版本').should('be.visible')
    })

    it('点击拦截记录 Tab 应显示拦截日志', () => {
      cy.get('main').contains('button', '拦截记录').click()
      cy.contains('已拦截').should('be.visible')
      cy.contains('已脱敏').should('be.visible')
      cy.contains('wang.fang').should('be.visible')
    })

    it('切换回 DLP 规则 Tab 应恢复原内容', () => {
      cy.get('main').contains('button', '敏感词典').click()
      cy.contains('身份证号码库').should('be.visible')
      cy.get('main').contains('button', 'DLP 规则').click()
      cy.contains('双向扫描已启用').should('be.visible')
      cy.contains('身份证号码检测').should('be.visible')
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
