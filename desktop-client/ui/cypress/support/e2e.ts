// Cypress E2E support file
// 在所有测试文件之前自动加载

import './commands';

// 忽略 Tauri 环境中预期的错误
Cypress.on('uncaught:exception', (err) => {
  // Tauri IPC 在浏览器测试环境中不可用，忽略相关错误
  if (
    err.message.includes('__TAURI__') ||
    err.message.includes('tauri') ||
    err.message.includes('ResizeObserver')
  ) {
    return false;
  }
  return true;
});
