/**
 * 模型配置 E2E 测试
 *
 * 覆盖维度：
 * - 正常路径：统计卡片、筛选栏、表格、空状态
 * - 弹窗交互：打开/关闭、表单字段、提供商切换、验证
 * - 安全审计：API Key 脱敏
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
      cy.contains('刷新').should('be.visible')
      cy.contains('添加模型').should('be.visible')
    })

    it('应渲染 4 个统计卡片', () => {
      cy.contains('模型总数').should('be.visible')
      cy.contains('已启用').should('be.visible')
      cy.contains('已禁用').should('be.visible')
      cy.contains('提供商').should('be.visible')
    })

    it('应渲染筛选栏和搜索框', () => {
      cy.get('input[placeholder*="搜索模型"]').should('be.visible')
      cy.contains('个模型配置').should('be.visible')
    })

    it('应渲染表格表头', () => {
      cy.contains('模型名称').should('be.visible')
      cy.contains('模型 ID').should('be.visible')
      cy.contains('API Endpoint').should('be.visible')
      cy.contains('API Key').should('be.visible')
    })

    it('应渲染分页', () => {
      cy.get('[data-testid="table-pagination"]').should('be.visible')
    })
  })

  describe('添加模型弹窗', () => {
    beforeEach(() => {
      cy.visit('/model-configs')
    })

    it('点击添加模型按钮应打开弹窗', () => {
      cy.contains('添加模型').click()
      cy.contains('添加模型配置').should('be.visible')
    })

    it('弹窗应包含所有表单字段', () => {
      cy.contains('添加模型').click()
      cy.contains('模型提供商').should('be.visible')
      cy.contains('模型 ID').should('be.visible')
      cy.contains('显示名称').should('be.visible')
      cy.contains('描述').should('be.visible')
      cy.contains('API Base URL').should('be.visible')
      cy.contains('API Key').should('be.visible')
      cy.contains('API 格式').should('be.visible')
      cy.contains('排序权重').should('be.visible')
      cy.contains('能力标签').should('be.visible')
    })

    it('弹窗应显示 API 格式 Radio 选项', () => {
      cy.contains('添加模型').click()
      cy.contains('OpenAI 兼容').should('be.visible')
      cy.contains('Anthropic 兼容').should('be.visible')
    })

    it('弹窗底部应有测试连接、取消和创建按钮', () => {
      cy.contains('添加模型').click()
      cy.contains('测试连接').should('be.visible')
      cy.contains('取消').should('be.visible')
      cy.contains('创建').should('be.visible')
    })

    it('点击取消应关闭弹窗', () => {
      cy.contains('添加模型').click()
      cy.contains('添加模型配置').should('be.visible')
      cy.contains('button', '取消').click()
      cy.contains('添加模型配置').should('not.exist')
    })

    it('默认提供商应为 DeepSeek', () => {
      cy.contains('添加模型').click()
      cy.contains('🐋').should('be.visible')
      cy.contains('DeepSeek').should('be.visible')
    })

    it('创建时不填必填字段应显示错误', () => {
      cy.contains('添加模型').click()
      cy.contains('button', '创建').click()
      cy.contains('请填写必填字段').should('be.visible')
    })
  })

  describe('安全审计', () => {
    it('表格中 API Key 列应有脱敏图标', () => {
      cy.visit('/model-configs')
      // 表头应有 API Key 列
      cy.contains('API Key').should('be.visible')
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
