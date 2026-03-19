/// <reference types="cypress" />

/**
 * 自定义 Cypress 命令
 * 用于简化测试代码和提高可复用性
 */

// 登录命令
Cypress.Commands.add('login', (username: string, password: string) => {
  cy.visit('/login');
  cy.get('input[name="username"]').type(username);
  cy.get('input[type="password"]').type(password);
  cy.get('button[type="submit"]').click();
  cy.wait(2000);
});

// 登出命令
Cypress.Commands.add('logout', () => {
  cy.get('.user-menu').click();
  cy.contains('退出登录').click();
  cy.wait(500);
});

// 清除所有数据命令
Cypress.Commands.add('cleanupData', () => {
  cy.clearLocalStorage();
  cy.clearCookies();
});

// 等待 API 请求完成命令
Cypress.Commands.add('waitForApi', (alias: string, timeout = 10000) => {
  cy.wait(alias, { timeout });
});

// 验证令牌存在命令
Cypress.Commands.add('verifyToken', () => {
  cy.window().then((win) => {
    const token = win.localStorage.getItem('token');
    expect(token).to.exist;
    expect(token).to.have.length.greaterThan(20);
  });
});

// 设置令牌命令（用于跳过登录）
Cypress.Commands.add('setToken', (token: string) => {
  cy.window().then((win) => {
    win.localStorage.setItem('token', token);
  });
});

// 验证在仪表盘页面命令
Cypress.Commands.add('verifyDashboard', () => {
  cy.url().should('include', '/dashboard');
  cy.get('.dashboard-container', { timeout: 10000 }).should('be.visible');
});

// 验证错误提示命令
Cypress.Commands.add('verifyError', (message: string) => {
  cy.get('.error-message', { timeout: 5000 }).should('be.visible');
  cy.get('.error-message').should('contain', message);
});

// 验证成功提示命令
Cypress.Commands.add('verifySuccess', (message: string) => {
  cy.get('.success-message', { timeout: 5000 }).should('be.visible');
  cy.get('.success-message').should('contain', message);
});

// 导航到指定页面命令
Cypress.Commands.add('navigateTo', (page: string) => {
  cy.get('.sidebar-menu').contains(page).click();
  cy.wait(500);
});

// 打开模态框命令
Cypress.Commands.add('openModal', (buttonText: string) => {
  cy.contains('button', buttonText).click();
  cy.get('.modal', { timeout: 5000 }).should('be.visible');
});

// 关闭模态框命令
Cypress.Commands.add('closeModal', () => {
  cy.get('.modal-close').click();
  cy.get('.modal').should('not.exist');
});

// 填写表单命令
Cypress.Commands.add('fillForm', (formData: Record<string, string>) => {
  Object.entries(formData).forEach(([field, value]) => {
    cy.get(`[name="${field}"]`).clear().type(value);
  });
});

// 提交表单命令
Cypress.Commands.add('submitForm', () => {
  cy.get('button[type="submit"]').click();
  cy.wait(1000);
});

// 验证表格行数命令
Cypress.Commands.add('verifyTableRows', (count: number) => {
  cy.get('table tbody tr').should('have.length', count);
});

// 搜索命令
Cypress.Commands.add('search', (query: string) => {
  cy.get('.search-input').clear().type(query);
  cy.wait(500);
});

// 验证加载状态命令
Cypress.Commands.add('verifyLoading', () => {
  cy.get('.loading-spinner').should('be.visible');
});

// 等待加载完成命令
Cypress.Commands.add('waitForLoading', () => {
  cy.get('.loading-spinner', { timeout: 10000 }).should('not.exist');
});

// 声明自定义命令的类型
declare global {
  namespace Cypress {
    interface Chainable {
      login(username: string, password: string): Chainable<void>;
      logout(): Chainable<void>;
      cleanupData(): Chainable<void>;
      waitForApi(alias: string, timeout?: number): Chainable<void>;
      verifyToken(): Chainable<void>;
      setToken(token: string): Chainable<void>;
      verifyDashboard(): Chainable<void>;
      verifyError(message: string): Chainable<void>;
      verifySuccess(message: string): Chainable<void>;
      navigateTo(page: string): Chainable<void>;
      openModal(buttonText: string): Chainable<void>;
      closeModal(): Chainable<void>;
      fillForm(formData: Record<string, string>): Chainable<void>;
      submitForm(): Chainable<void>;
      verifyTableRows(count: number): Chainable<void>;
      search(query: string): Chainable<void>;
      verifyLoading(): Chainable<void>;
      waitForLoading(): Chainable<void>;
    }
  }
}

export {};
