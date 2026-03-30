/**
 * 知识库管理 E2E 测试
 *
 * 覆盖维度：
 * - 正常路径：页面渲染、知识库卡片、文档表格
 * - 导航：侧边栏跳转
 */

describe('知识库管理', () => {
  beforeEach(() => {
    cy.loginByState()
  })

  describe('正常路径', () => {
    beforeEach(() => {
      cy.visit('/knowledge-bases')
    })

    it('应渲染页面标题和操作按钮', () => {
      cy.contains('知识库管理').should('be.visible')
      cy.contains('管理企业私有知识库，增强 AI 助手回答能力').should('be.visible')
      cy.contains('新建知识库').should('be.visible')
    })

    it('应渲染 3 个知识库卡片', () => {
      cy.contains('产品文档库').should('be.visible')
      cy.contains('HR 政策库').should('be.visible')
      cy.contains('法务合规库').should('be.visible')
      cy.contains('已启用').should('be.visible')
      cy.contains('已禁用').should('be.visible')
      cy.contains('研发部可访问').should('be.visible')
      cy.contains('全员可访问').should('be.visible')
      cy.contains('法务部可访问').should('be.visible')
    })

    it('应渲染文档管理区域标题', () => {
      cy.contains('产品文档库 — 文档管理').should('be.visible')
      cy.contains('42 个文档').should('be.visible')
    })

    it('应渲染文档表格和 4 行 Mock 数据', () => {
      cy.contains('文件名').should('be.visible')
      cy.contains('类型').should('be.visible')
      cy.contains('切片').should('be.visible')
      cy.contains('上传时间').should('be.visible')
      cy.contains('产品需求规格说明书 v3.2.pdf').should('be.visible')
      cy.contains('API 接口设计文档.docx').should('be.visible')
      cy.contains('技术架构说明.md').should('be.visible')
      cy.contains('部署运维手册.txt').should('be.visible')
    })
  })

  describe('导航', () => {
    it('从侧边栏点击知识库能跳转', () => {
      cy.visit('/')
      cy.get('aside').contains('知识库').click()
      cy.url().should('include', '/knowledge-bases')
      cy.contains('管理企业私有知识库，增强 AI 助手回答能力').should('be.visible')
    })
  })
})
