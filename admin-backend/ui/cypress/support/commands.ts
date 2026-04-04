/// <reference types="cypress" />

const API_URL = () => Cypress.env('apiUrl') ?? 'http://localhost:3000/api'
const TEST_USER = () => Cypress.env('testUsername') ?? 'admin'
const TEST_PASS = () => Cypress.env('testPassword') ?? 'admin123'

/** 通过 UI 表单登录 */
Cypress.Commands.add('login', (username: string, password: string) => {
  cy.visit('/login')
  cy.get('input[placeholder="请输入用户名"]').type(username)
  cy.get('input[placeholder="请输入密码"]').type(password)
  cy.get('button[type="submit"]').click()
  cy.url().should('eq', Cypress.config().baseUrl + '/')
})

/**
 * 通过 cy.session 缓存登录状态，整个测试套件只调用一次登录 API。
 * session 在 beforeEach 里调用，Cypress 会自动复用已有 session。
 *
 * 使用方式：
 *   beforeEach(() => {
 *     cy.loginByState()
 *     cy.visit('/some-page')
 *   })
 */
Cypress.Commands.add('loginByState', (username?: string) => {
  const name = username ?? TEST_USER()

  cy.session(
    // session key：用户名变化时重新登录
    ['auth', name],
    () => {
      // setup：只在 session 不存在时执行（整个测试套件只跑一次）
      cy.request({
        method: 'POST',
        url: `${API_URL()}/auth/login`,
        body: { username: name, password: TEST_PASS() },
        failOnStatusCode: false,
      }).then((resp) => {
        const token =
          resp.status === 200 && resp.body?.access_token
            ? resp.body.access_token
            : `dev-mock-token-${Date.now()}`

        const user =
          resp.status === 200 && resp.body?.user
            ? {
                id: resp.body.user.id,
                username: resp.body.user.username,
                email: resp.body.user.email,
                role: resp.body.user.roles?.[0] ?? '超级管理员',
                mfa_enabled: false,
                status: 'active',
                created_at: new Date().toISOString(),
              }
            : {
                id: 'dev-1',
                username: name,
                email: `${name}@ironclaw.dev`,
                role: '超级管理员',
                mfa_enabled: false,
                status: 'active',
                created_at: new Date().toISOString(),
              }

        const state = JSON.stringify({
          state: { token, user, isAuthenticated: true },
          version: 0,
        })

        // session setup 需要先有页面上下文才能写 localStorage
        cy.visit('/')
        cy.window().then((win) => {
          win.localStorage.setItem('ironclaw-auth', state)
          win.localStorage.setItem('auth_token', token)
        })
      })
    },
    {
      // validate：每次 beforeEach 时验证 session 是否仍有效
      validate() {
        cy.window().then((win) => {
          const stored = win.localStorage.getItem('ironclaw-auth')
          expect(stored, 'auth state should exist in localStorage').to.exist
          const parsed = JSON.parse(stored!)
          expect(parsed.state.isAuthenticated).to.be.true
        })
      },
    }
  )
})

/** 退出登录 */
Cypress.Commands.add('logout', () => {
  cy.clearLocalStorage()
  cy.visit('/login')
})

declare global {
  namespace Cypress {
    interface Chainable {
      login(username: string, password: string): Chainable<void>
      loginByState(username?: string): Chainable<void>
      logout(): Chainable<void>
    }
  }
}

export {}
