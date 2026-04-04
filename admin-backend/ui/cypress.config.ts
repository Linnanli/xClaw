import { defineConfig } from 'cypress'

/**
 * E2E 测试说明：
 * 后端需要以较高的限流阈值启动，否则测试会触发 429：
 *   RATE_LIMIT_DEFAULT=1000 cargo run -p admin-backend
 */
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
    setupNodeEvents(on) {
      on('task', {
        log(message) {
          console.log(message)
          return null
        },
      })
    },
    env: {
      apiUrl: 'http://localhost:3000/api',
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
})
