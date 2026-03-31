/**
 * 水印管理 E2E 测试
 */

describe('水印管理', () => {
  beforeEach(() => {
    cy.loginByState()
  })

  describe('正常路径', () => {
    beforeEach(() => {
      cy.visit('/watermark')
    })

    it('应渲染页面标题', () => {
      cy.contains('水印管理').should('be.visible')
      cy.contains('配置导出文件的可见水印').should('be.visible')
    })

    it('应渲染水印配置区域', () => {
      cy.contains('水印配置').should('be.visible')
      cy.contains('内容模板').should('be.visible')
      cy.contains('字体大小').should('be.visible')
      cy.contains('透明度').should('be.visible')
      cy.contains('位置').should('be.visible')
    })

    it('应渲染预览区域', () => {
      cy.contains('预览效果').should('be.visible')
    })

    it('应有保存按钮', () => {
      cy.contains('保存配置').should('be.visible')
    })
  })

  describe('导航', () => {
    it('从侧边栏点击水印追踪能跳转', () => {
      cy.visit('/')
      cy.get('aside').contains('水印追踪').click()
      cy.url().should('include', '/watermark')
    })
  })
})
