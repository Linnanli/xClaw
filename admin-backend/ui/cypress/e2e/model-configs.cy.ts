/**
 * 模型配置 E2E 测试
 *
 * 覆盖维度：
 * - 正常路径：页面渲染、模型表格
 * - 导航：URL 直接访问
 */

describe('模型配置', () => {
  beforeEach(() => {
    cy.loginByState()
  })

  describe('正常路径', () => {
    beforeEach(() => {
      cy.visit('/model-configs')
    })

    it('应渲染页面标题和操作按钮', () => {
      cy.contains('模型配置').should('be.visible')
      cy.contains('管理多提供商 AI 模型的接入配置与调度').should('be.visible')
      cy.contains('添加模型').should('be.visible')
    })

    it('应渲染表格表头', () => {
      cy.contains('模型名称').should('be.visible')
      cy.contains('提供商').should('be.visible')
      cy.contains('API 格式').should('be.visible')
      cy.contains('能力标签').should('be.visible')
    })

    it('应渲染 4 行模型数据', () => {
      cy.contains('DeepSeek-V3').should('be.visible')
      cy.contains('Qwen-Max').should('be.visible')
      cy.contains('GPT-4o').should('be.visible')
      cy.contains('Claude-3.5').should('be.visible')
      cy.contains('已启用').should('have.length.at.least', 1)
      cy.contains('已禁用').should('be.visible')
      cy.contains('✓ 默认').should('be.visible')
    })
  })

  describe('导航', () => {
    it('通过 URL 直接访问模型配置', () => {
      cy.visit('/model-configs')
      cy.contains('模型配置').should('be.visible')
      cy.contains('管理多提供商 AI 模型的接入配置与调度').should('be.visible')
    })
  })
})
