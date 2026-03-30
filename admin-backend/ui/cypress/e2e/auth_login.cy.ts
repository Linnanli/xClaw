/**
 * 认证登录 E2E 测试
 *
 * 覆盖维度：
 * - 正常路径：成功登录、页面跳转
 * - 失败路径：空表单、错误凭据
 * - 安全审计：密码隐藏、token 存储
 * - UI 还原：设计图元素验证
 */

describe('认证登录', () => {
  beforeEach(() => {
    cy.clearLocalStorage()
    cy.visit('/login')
  })

  /* ── 正常路径 ── */

  describe('正常路径', () => {
    it('应渲染登录页所有核心元素', () => {
      // 左侧品牌区
      cy.contains('IRONCLAW').should('be.visible')
      cy.contains('企业级').should('be.visible')
      cy.contains('所有服务运行正常').should('be.visible')

      // 右侧表单
      cy.contains('管理员登录').should('be.visible')
      cy.get('input[placeholder="请输入用户名"]').should('be.visible')
      cy.get('input[placeholder="请输入密码"]').should('be.visible')
      cy.get('input[placeholder="6 位 TOTP 验证码"]').should('be.visible')
      cy.contains('登录').should('be.visible')
      cy.contains('LDAP / AD').should('be.visible')
      cy.contains('OIDC / SAML').should('be.visible')
      cy.contains('TLS 1.3').should('be.visible')
    })

    it('应成功登录并跳转到仪表盘', () => {
      cy.get('input[placeholder="请输入用户名"]').type('admin')
      cy.get('input[placeholder="请输入密码"]').type('admin123')
      cy.get('button[type="submit"]').click()

      // 开发模式 mock 登录，应跳转到首页
      cy.url().should('eq', Cypress.config().baseUrl + '/')
      cy.contains('总览').should('be.visible')
    })

    it('登录后 token 应保存到 localStorage', () => {
      cy.get('input[placeholder="请输入用户名"]').type('admin')
      cy.get('input[placeholder="请输入密码"]').type('admin123')
      cy.get('button[type="submit"]').click()

      cy.url().should('eq', Cypress.config().baseUrl + '/').then(() => {
        const token = localStorage.getItem('auth_token')
        expect(token).to.exist
        expect(token!.length).to.be.greaterThan(5)
      })
    })

    it('已登录用户访问 /login 应重定向到首页', () => {
      cy.loginByState()
      cy.visit('/login')
      cy.url().should('eq', Cypress.config().baseUrl + '/')
    })
  })

  /* ── 失败路径 ── */

  describe('失败路径', () => {
    it('空用户名提交应显示错误', () => {
      cy.get('input[placeholder="请输入密码"]').type('password')
      cy.get('button[type="submit"]').click()
      cy.contains('请输入用户名').should('be.visible')
    })

    it('空密码提交应显示错误', () => {
      cy.get('input[placeholder="请输入用户名"]').type('admin')
      cy.get('button[type="submit"]').click()
      cy.contains('请输入密码').should('be.visible')
    })

    it('只有空格的用户名应视为空', () => {
      cy.get('input[placeholder="请输入用户名"]').type('   ')
      cy.get('input[placeholder="请输入密码"]').type('password')
      cy.get('button[type="submit"]').click()
      cy.contains('请输入用户名').should('be.visible')
    })
  })

  /* ── 安全审计 ── */

  describe('安全审计', () => {
    it('密码输入框应为 password 类型', () => {
      cy.get('input[placeholder="请输入密码"]').should('have.attr', 'type', 'password')
    })

    it('URL 中不应包含密码', () => {
      cy.get('input[placeholder="请输入用户名"]').type('admin')
      cy.get('input[placeholder="请输入密码"]').type('secret123')
      cy.get('button[type="submit"]').click()
      cy.url().should('not.contain', 'secret123')
    })
  })
})
