// ***********************************************************
// This example support/e2e.ts is processed and
// loaded automatically before your test files.
//
// This is a great place to put global configuration and
// behavior that modifies Cypress.
//
// You can change the location of this file or turn off
// automatically serving support files with the
// 'supportFile' configuration option.
//
// You can read more here:
// https://on.cypress.io/configuration
// ***********************************************************

// Import commands.js using ES2015 syntax:
import './commands';

// Alternatively you can use CommonJS syntax:
// require('./commands')

// 全局配置
Cypress.on('uncaught:exception', (err, runnable) => {
  // 忽略某些预期的错误
  if (err.message.includes('ResizeObserver')) {
    return false;
  }
  
  // 让其他错误正常抛出
  return true;
});

// 全局 beforeEach
beforeEach(() => {
  // 设置默认超时
  Cypress.config('defaultCommandTimeout', 10000);
  
  // 清除控制台
  cy.window().then((win) => {
    win.console.clear();
  });
});

// 全局 afterEach
afterEach(() => {
  // 截图失败的测试
  cy.screenshot({ capture: 'runner', onlyOnFailure: true });
});
