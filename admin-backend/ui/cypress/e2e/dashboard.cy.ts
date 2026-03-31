/**
 * 仪表盘 E2E 测试
 *
 * 覆盖维度：
 * - 正常路径：页面渲染、数据展示
 * - 失败路径：未登录重定向
 * - UI 还原：设计图元素验证
 */

describe('仪表盘', () => {
  beforeEach(() => {
    cy.loginByState()
    cy.visit('/')
  })

  describe('正常路径', () => {
    it('应渲染页面标题和操作按钮', () => {
      cy.contains('总览').should('be.visible')
      cy.contains('监控 DLP、客户端、AI 使用与安全指标').should('be.visible')
      cy.contains('刷新').should('be.visible')
      cy.contains('导出').should('be.visible')
    })

    it('应渲染 6 个统计卡片', () => {
      cy.contains('总用户数').should('be.visible')
      cy.contains('在线客户端').should('be.visible')
      cy.contains('DLP 拦截').should('be.visible')
      cy.contains('今日费用').should('be.visible')
      cy.contains('敏感操作').should('be.visible')
      cy.contains('系统健康度').should('be.visible')
    })

    it('应渲染图表区域', () => {
      cy.contains('用户活动趋势').should('be.visible')
      cy.contains('DLP 拦截趋势').should('be.visible')
      cy.contains('近 7 天').should('be.visible')
    })

    it('应渲染最近操作记录表格', () => {
      cy.contains('最近操作记录').should('be.visible')
      cy.contains('查看全部').should('be.visible')
      // 表头
      cy.contains('时间').should('be.visible')
      cy.contains('操作人').should('be.visible')
      cy.contains('操作类型').should('be.visible')
      cy.contains('状态').should('be.visible')
    })
  })

  describe('侧边栏导航', () => {
    it('应渲染 Logo 和系统状态', () => {
      cy.get('aside').within(() => {
        cy.contains('IC').should('be.visible')
        cy.contains('IRONCLAW').should('be.visible')
        cy.contains('运行时间:').should('be.visible')
        cy.contains('99.97%').should('be.visible')
        cy.contains('DLP状态:').should('be.visible')
        cy.contains('运行中').should('be.visible')
      })
    })

    it('应渲染 17 个导航项', () => {
      const items = [
        '总览', '客户端', '用户管理', '部门管理', '安全策略', '告警中心',
        '对话审计', '知识库', '模型配置', '费用管理', '扩展管理', '合规管理',
        '水印追踪', '审批工单', '审计日志', '统计报表', '系统设置',
      ]
      cy.get('aside').within(() => {
        items.forEach((item) => cy.contains(item).should('be.visible'))
      })
    })

    it('总览应为当前高亮菜单', () => {
      // 活跃菜单项的文字应该比非活跃项更深色
      cy.get('aside').contains('a', '总览').find('span').should('have.css', 'font-weight', '600')
      cy.get('aside').contains('a', '客户端').find('span').should('have.css', 'font-weight', '500')
    })

    it('应渲染底部用户信息', () => {
      cy.get('aside').within(() => {
        cy.contains('admin').should('be.visible')
        cy.contains('超级管理员').should('be.visible')
      })
    })
  })

  describe('顶部栏', () => {
    it('应渲染面包屑', () => {
      cy.get('header').within(() => {
        cy.contains('系统').should('be.visible')
        cy.contains('总览').should('be.visible')
      })
    })
  })

  describe('失败路径', () => {
    it('未登录访问首页应重定向到登录页', () => {
      cy.clearLocalStorage()
      cy.visit('/')
      cy.url().should('include', '/login')
    })
  })
})
