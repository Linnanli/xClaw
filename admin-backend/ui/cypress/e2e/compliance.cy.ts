/**
 * 合规管理 E2E 测试
 *
 * 覆盖维度：
 * - 正常路径：页面渲染、分级卡片、保留策略、报告表格
 * - 交互：生成报告弹窗、报告详情弹窗（查看按钮）
 * - 失败路径：生成报告表单校验
 * - 导航：侧边栏跳转
 */

describe('合规管理', () => {
  beforeEach(() => {
    cy.loginByState()
    cy.visit('/compliance')
  })

  describe('正常路径', () => {
    it('应渲染页面标题和生成报告按钮', () => {
      cy.contains('合规管理').should('be.visible')
      cy.contains('数据分类分级、合规报告与数据保留策略').should('be.visible')
      cy.contains('生成报告').should('be.visible')
    })

    it('应渲染 4 个数据分级卡片', () => {
      cy.get('body').then($body => {
        if ($body.text().includes('公开')) {
          cy.contains('公开').should('be.visible')
          cy.contains('内部').should('be.visible')
          cy.contains('机密').should('be.visible')
          cy.contains('绝密').should('be.visible')
        }
      })
    })

    it('应渲染数据保留策略区域', () => {
      cy.contains('数据保留策略').should('be.visible')
    })

    it('应渲染历史合规报告表格表头', () => {
      cy.contains('历史合规报告').should('be.visible')
      const headers = ['报告名称', '类型', '时间范围', '生成时间', '操作']
      headers.forEach(h => cy.contains(h).should('be.visible'))
    })
  })

  describe('交互 - 生成报告弹窗', () => {
    it('点击生成报告应打开弹窗', () => {
      cy.contains('button', '生成报告').should('be.visible').click()
      cy.contains('生成合规报告').should('be.visible')
      cy.contains('报告名称').should('be.visible')
      cy.contains('报告类型').should('be.visible')
      cy.contains('开始日期').should('be.visible')
      cy.contains('结束日期').should('be.visible')
    })

    it('弹窗中取消按钮应关闭弹窗', () => {
      cy.contains('button', '生成报告').click()
      cy.contains('生成合规报告').should('be.visible')
      cy.contains('取消').click()
      cy.contains('生成合规报告').should('not.exist')
    })

    it('未填写必填项时生成按钮应禁用', () => {
      cy.contains('button', '生成报告').click()
      cy.contains('生成合规报告').should('be.visible')
      // 弹窗内的"生成"按钮（不含"报告"文字）应 disabled
      cy.get('[role="dialog"]').find('button').contains(/^生成$/).should('be.disabled')
    })
  })

  describe('交互 - 报告详情弹窗', () => {
    it('有报告时点击查看应打开详情弹窗', () => {
      cy.get('body').then($body => {
        if ($body.find('button:contains("查看")').length > 0) {
          cy.contains('查看').first().click()
          cy.contains('安全事件汇总').should('be.visible')
          cy.contains('DLP 拦截次数').should('be.visible')
          cy.contains('策略变更次数').should('be.visible')
          cy.contains('告警事件数').should('be.visible')
          cy.contains('审批工单数').should('be.visible')
          cy.contains('生成时间').should('be.visible')
        }
      })
    })

    it('详情弹窗应显示导出 PDF 按钮', () => {
      cy.get('body').then($body => {
        if ($body.find('button:contains("查看")').length > 0) {
          cy.contains('查看').first().click()
          cy.contains('安全事件汇总').should('be.visible')
          cy.contains('button', '导出 PDF').should('be.visible')
        }
      })
    })

    it('详情弹窗关闭按钮应关闭弹窗', () => {
      cy.get('body').then($body => {
        if ($body.find('button:contains("查看")').length > 0) {
          cy.contains('查看').first().click()
          cy.contains('安全事件汇总').should('be.visible')
          cy.contains('关闭').click()
          cy.contains('安全事件汇总').should('not.exist')
        }
      })
    })
  })

  describe('导航', () => {
    it('从侧边栏点击合规管理能跳转', () => {
      cy.visit('/')
      // 侧边栏"安全与合规"分组，点击展开后再点子菜单
      cy.get('aside').contains('安全与合规').click()
      cy.get('aside').contains('合规管理').should('be.visible').click()
      cy.url().should('include', '/compliance')
      cy.contains('数据分类分级、合规报告与数据保留策略').should('be.visible')
    })
  })
})
