/// <reference types="cypress" />

// desktop-client E2E 自定义命令

declare global {
  namespace Cypress {
    interface Chainable {
      // 预留扩展点
    }
  }
}

export {};
