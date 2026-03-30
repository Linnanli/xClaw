/// <reference types="cypress" />

/** 通过 UI 表单登录 */
Cypress.Commands.add('login', (username: string, password: string) => {
  cy.visit('/login')
  cy.get('input[placeholder="请输入用户名"]').type(username)
  cy.get('input[placeholder="请输入密码"]').type(password)
  cy.get('button[type="submit"]').click()
  cy.url().should('eq', Cypress.config().baseUrl + '/')
})

/** 通过 localStorage 直接注入认证状态（跳过登录 UI） */
Cypress.Commands.add('loginByState', (username?: string) => {
  const name = username ?? 'admin'
  const state = {
    state: {
      token: `dev-mock-token-${Date.now()}`,
      user: {
        id: 'dev-1',
        username: name,
        email: `${name}@ironclaw.dev`,
        role: '超级管理员',
        mfa_enabled: false,
        status: 'active',
        created_at: new Date().toISOString(),
      },
      isAuthenticated: true,
    },
    version: 0,
  }
  localStorage.setItem('ironclaw-auth', JSON.stringify(state))
  localStorage.setItem('auth_token', state.state.token)
})

/** 退出登录 */
Cypress.Commands.add('logout', () => {
  localStorage.removeItem('ironclaw-auth')
  localStorage.removeItem('auth_token')
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
