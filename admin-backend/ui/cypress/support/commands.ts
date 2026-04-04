/// <reference types="cypress" />

const API_URL = () => Cypress.env('apiUrl') ?? 'http://localhost:3000/api'
const TEST_USER = () => Cypress.env('testUsername') ?? 'admin'
const TEST_PASS = () => Cypress.env('testPassword') ?? 'admin123'

Cypress.Commands.add('login', (username: string, password: string) => {
  cy.visit('/login')
  cy.get('input[placeholder="请输入用户名"]').type(username)
  cy.get('input[placeholder="请输入密码"]').type(password)
  cy.get('button[type="submit"]').click()
  cy.url().should('eq', Cypress.config().baseUrl + '/')
})

Cypress.Commands.add('loginByState', (username?: string) => {
  const name = username ?? TEST_USER()

  cy.session(
    ['auth', name],
    () => {
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

        cy.visit('/')
        cy.window().then((win) => {
          win.localStorage.setItem('auth_token', token)
        })
      })
    },
    {
      validate() {
        cy.window().then((win) => {
          expect(win.localStorage.getItem('auth_token'), 'auth_token should exist').to.exist
        })
      },
    }
  )
})

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
