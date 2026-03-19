import { defineConfig } from 'cypress';

export default defineConfig({
  e2e: {
    baseUrl: 'http://localhost:5174',
    viewportWidth: 1920,
    viewportHeight: 1080,
    defaultCommandTimeout: 10000,
    requestTimeout: 10000,
    responseTimeout: 10000,
    video: false,
    screenshotOnRunFailure: true,
    setupNodeEvents(on, config) {
      // 实现 node 事件监听器
      on('task', {
        log(message) {
          console.log(message);
          return null;
        },
      });
    },
    env: {
      // 后端 API 地址
      apiUrl: 'http://localhost:3000/api',
      // 测试用户凭据
      testUsername: 'admin',
      testPassword: 'admin123',
    },
  },
  component: {
    devServer: {
      framework: 'react',
      bundler: 'vite',
    },
  },
});
