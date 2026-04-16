/**
 * 审计日志 E2E 测试
 *
 * 覆盖维度：
 * - 正常路径：页面渲染、筛选栏控件存在性、日志表格、详情 Dialog
 * - 过滤交互：操作类型 select、全文搜索、日期范围
 * - 导航：URL 直接访问
 */

const MOCK_LOG = {
  id: '00000000-0000-0000-0000-000000000001',
  username: 'admin',
  user_id: '00000000-0000-0000-0000-000000000099',
  action: 'create_user',
  details: '创建用户: alice',
  ip_address: '127.0.0.1',
  user_agent: 'Mozilla/5.0',
  created_at: '2024-01-01T10:00:00Z',
}

const MOCK_RESPONSE = {
  logs: [MOCK_LOG],
  total: 1,
  page: 1,
  page_size: 10,
  total_pages: 1,
}

describe('审计日志', () => {
  beforeEach(() => {
    cy.loginByState()
    cy.intercept('GET', '/api/audit-logs*', { body: MOCK_RESPONSE }).as('getLogs')
  })

  describe('正常路径', () => {
    beforeEach(() => {
      cy.visit('/audit-logs')
      cy.wait('@getLogs')
    })

    it('应渲染页面标题和描述', () => {
      cy.contains('审计日志').should('be.visible')
      cy.contains('查看和导出所有管理操作的审计记录').should('be.visible')
    })

    it('应渲染筛选栏控件', () => {
      cy.get('input[placeholder="搜索操作或详情..."]').should('exist')
      cy.get('select').contains('全部操作').should('exist')
      cy.get('input[type="date"]').should('have.length.at.least', 2)
      cy.contains('导出 CSV').should('be.visible')
    })

    it('应渲染操作类型下拉选项', () => {
      cy.get('select').first().find('option').should('contain', '登录成功')
      cy.get('select').first().find('option').should('contain', '创建用户')
      cy.get('select').first().find('option').should('contain', '删除用户')
    })

    it('应渲染模拟日志行数据', () => {
      cy.contains('创建用户').should('be.visible')
      cy.contains('创建用户: alice').should('be.visible')
      cy.contains('admin').should('be.visible')
      cy.contains('127.0.0.1').should('be.visible')
    })
  })

  describe('服务端过滤', () => {
    beforeEach(() => {
      cy.visit('/audit-logs')
      cy.wait('@getLogs')
    })

    it('选择操作类型后应携带 action 参数', () => {
      cy.intercept('GET', '/api/audit-logs*', { body: { ...MOCK_RESPONSE, logs: [] } }).as('filtered')
      cy.get('select').first().select('login_success')
      cy.wait('@filtered').its('request.url').should('include', 'action=login_success')
    })

    it('输入搜索文本后应携带 q 参数', () => {
      cy.intercept('GET', '/api/audit-logs*', { body: { ...MOCK_RESPONSE, logs: [] } }).as('searched')
      cy.get('input[placeholder="搜索操作或详情..."]').type('alice')
      cy.wait('@searched').its('request.url').should('include', 'q=alice')
    })
  })

  describe('详情 Dialog', () => {
    beforeEach(() => {
      cy.visit('/audit-logs')
      cy.wait('@getLogs')
    })

    it('点击详情应打开 Dialog 显示日志信息', () => {
      cy.contains('详情').click()
      cy.contains('日志详情').should('be.visible')
      cy.contains('127.0.0.1').should('be.visible')
      cy.contains('Mozilla/5.0').should('be.visible')
    })

    it('关闭 Dialog 后应消失', () => {
      cy.contains('详情').click()
      cy.contains('日志详情').should('be.visible')
      cy.get('button').contains('✕').click()
      cy.contains('日志详情').should('not.exist')
    })
  })
})
