/**
 * 扩展管理 E2E（匹配新版表格与上传流程）
 */

type SkillStatus = 'pending' | 'approved' | 'rejected' | 'scan_failed' | 'yanked' | 'scanning'

interface SkillRow {
  id: string
  name: string
  description: string
  version: string
  author: string
  enabled: boolean
  source: 'builtin' | 'admin_upload'
  review_status: SkillStatus
  is_builtin: boolean
  invoke_count: number
  updated_at: string
}

interface PluginRow {
  id: string
  name: string
  description: string
  version: string
  author: string
  enabled: boolean
  source: 'builtin' | 'admin_upload'
  review_status: SkillStatus
  plugin_type: 'http' | 'stdio' | 'wasm'
  is_builtin: boolean
  invoke_count: number
  requires_sandbox: boolean
  updated_at: string
}

const now = () => new Date().toISOString()

function mockSkills(skills: SkillRow[]) {
  cy.intercept('GET', '/api/skills', {
    body: {
      skills,
      count: skills.length,
    },
  }).as('getSkills')
}

function mockPlugins(plugins: PluginRow[]) {
  cy.intercept('GET', '/api/plugins', {
    body: {
      plugins,
      count: plugins.length,
    },
  }).as('getPlugins')
}

function seedSkills(): SkillRow[] {
  return [
    {
      id: 'skill-delegation',
      name: 'delegation',
      description: 'Helps users delegate tasks',
      version: '0.1.0',
      author: 'ironclaw',
      enabled: true,
      source: 'builtin',
      review_status: 'approved',
      is_builtin: true,
      invoke_count: 12,
      updated_at: now(),
    },
    {
      id: 'skill-pending',
      name: 'pending-skill',
      description: '待审核技能',
      version: '1.0.0',
      author: 'user',
      enabled: false,
      source: 'admin_upload',
      review_status: 'pending',
      is_builtin: false,
      invoke_count: 0,
      updated_at: now(),
    },
    {
      id: 'skill-scan-failed',
      name: 'scan-failed-skill',
      description: '扫描失败技能',
      version: '1.0.0',
      author: 'user',
      enabled: false,
      source: 'admin_upload',
      review_status: 'scan_failed',
      is_builtin: false,
      invoke_count: 1,
      updated_at: now(),
    },
    {
      id: 'skill-approved-upload',
      name: 'approved-skill',
      description: '已通过技能',
      version: '1.1.0',
      author: 'user',
      enabled: true,
      source: 'admin_upload',
      review_status: 'approved',
      is_builtin: false,
      invoke_count: 3,
      updated_at: now(),
    },
  ]
}

function seedPlugins(): PluginRow[] {
  return [
    {
      id: 'plugin-notion',
      name: 'notion',
      description: 'Notion 插件',
      version: '1.0.0',
      author: 'ironclaw',
      enabled: true,
      source: 'builtin',
      review_status: 'approved',
      plugin_type: 'http',
      is_builtin: true,
      invoke_count: 9,
      requires_sandbox: false,
      updated_at: now(),
    },
    {
      id: 'plugin-github',
      name: 'github',
      description: 'GitHub 插件',
      version: '1.2.0',
      author: 'ironclaw',
      enabled: true,
      source: 'builtin',
      review_status: 'approved',
      plugin_type: 'wasm',
      is_builtin: true,
      invoke_count: 15,
      requires_sandbox: false,
      updated_at: now(),
    },
    {
      id: 'plugin-pending',
      name: 'pending-plugin',
      description: '待审核插件',
      version: '1.0.0',
      author: 'user',
      enabled: false,
      source: 'admin_upload',
      review_status: 'pending',
      plugin_type: 'http',
      is_builtin: false,
      invoke_count: 0,
      requires_sandbox: false,
      updated_at: now(),
    },
    {
      id: 'plugin-approved-upload',
      name: 'approved-plugin',
      description: '已通过插件',
      version: '1.0.0',
      author: 'user',
      enabled: true,
      source: 'admin_upload',
      review_status: 'approved',
      plugin_type: 'http',
      is_builtin: false,
      invoke_count: 2,
      requires_sandbox: false,
      updated_at: now(),
    },
  ]
}

describe('扩展管理', () => {
  beforeEach(() => {
    cy.loginByState()
    mockSkills(seedSkills())
    mockPlugins(seedPlugins())
  })

  describe('页面结构与Tab', () => {
    beforeEach(() => {
      cy.visit('/extensions')
      cy.wait('@getSkills')
    })

    it('应渲染页面标题和说明', () => {
      cy.contains('扩展管理').should('be.visible')
      cy.contains('管理可用技能与插件扩展').should('be.visible')
    })

    it('应展示技能/插件两个Tab并默认在技能页', () => {
      cy.contains('button', '技能管理').should('be.visible')
      cy.contains('button', '插件管理').should('be.visible')
      cy.contains('上传技能包').should('be.visible')
    })

    it('切换到插件管理后应展示插件行', () => {
      cy.contains('button', '插件管理').click()
      cy.wait('@getPlugins')
      cy.contains('tr', 'notion').should('be.visible')
      cy.contains('tr', 'github').should('be.visible')
      cy.contains('上传技能包').should('not.exist')
    })
  })

  describe('技能列表展示', () => {
    beforeEach(() => {
      cy.visit('/extensions')
      cy.wait('@getSkills')
    })

    it('内置技能行应展示来源与审核状态', () => {
      cy.contains('tr', 'delegation').within(() => {
        cy.contains('内置').should('be.visible')
        cy.contains('已通过').should('be.visible')
      })
    })

    it('待审核技能应显示审核按钮', () => {
      cy.contains('tr', 'pending-skill').within(() => {
        cy.contains('button', '审核').should('be.visible')
      })
    })

    it('scan_failed 技能应显示重扫按钮', () => {
      cy.contains('tr', 'scan-failed-skill').within(() => {
        cy.contains('button', '重扫').should('be.visible')
      })
    })

    it('approved 上传技能应显示下架按钮', () => {
      cy.contains('tr', 'approved-skill').within(() => {
        cy.contains('button', '下架').should('be.visible')
      })
    })
  })

  describe('上传流程', () => {
    beforeEach(() => {
      cy.visit('/extensions')
      cy.wait('@getSkills')
    })

    it('未选择文件时应提示错误', () => {
      cy.contains('上传技能包').click()
      cy.contains('button', '开始上传').click()
      cy.contains('请先选择一个技能文件').should('be.visible')
    })

    it('选择不支持后缀应被前端拒绝', () => {
      cy.contains('上传技能包').click()
      cy.get('input[type="file"]').selectFile(
        {
          contents: Cypress.Buffer.from('---\nname: demo\n---\ncontent'),
          fileName: 'invalid-skill.md',
          mimeType: 'text/markdown',
        },
        { force: true },
      )
      cy.contains('button', '开始上传').click()
      cy.contains('仅支持 .zip / .tar.gz / .tgz 技能包').should('be.visible')
    })

    it('应通过 multipart 上传压缩包', () => {
      cy.intercept('POST', '/api/skills/upload-package', (req) => {
        const contentType = req.headers['content-type'] as string | undefined
        expect(contentType ?? '').to.contain('multipart/form-data')
        req.reply({
          statusCode: 201,
          body: {
            id: 'uploaded-id',
            name: 'demo-skill',
            review_status: 'pending',
          },
        })
      }).as('uploadPackage')

      cy.contains('上传技能包').click()
      cy.get('input[type="file"]').selectFile(
        {
          contents: Cypress.Buffer.from('PK\u0003\u0004fake zip'),
          fileName: 'good-skill.zip',
          mimeType: 'application/zip',
        },
        { force: true },
      )
      cy.contains('button', '开始上传').click()
      cy.wait('@uploadPackage')
      cy.contains('状态：pending').should('be.visible')
    })
  })

  describe('审核与扫描结果', () => {
    it('点击审核后应打开弹窗并拉取扫描结果', () => {
      mockSkills([
        {
          id: 'test-scan-skill-id',
          name: 'scan-skill',
          description: '待审核技能',
          version: '1.0.0',
          author: 'user',
          enabled: false,
          source: 'admin_upload',
          review_status: 'pending',
          is_builtin: false,
          invoke_count: 0,
          updated_at: now(),
        },
      ])

      cy.intercept('GET', '/api/skills/test-scan-skill-id/scan-results', {
        body: {
          skill_id: 'test-scan-skill-id',
          scan_result: {
            scanner_type: 'cisco-ai-skill-scanner',
            verdict: 'SUSPICIOUS',
            is_safe: false,
            max_severity: 'HIGH',
            findings_count: 1,
            findings: [{
              rule_id: 'vetter-001',
              severity: 'HIGH',
              title: '检测到可疑外发行为',
              location: 'SKILL.md:23',
              recommendation: '请移除直接外发请求',
            }],
            created_at: now(),
          },
        },
      }).as('getScanResults')

      cy.visit('/extensions')
      cy.wait('@getSkills')
      cy.contains('tr', 'scan-skill').within(() => {
        cy.contains('button', '审核').click()
      })
      cy.wait('@getScanResults')
      cy.contains('审核：scan-skill').should('be.visible')
      cy.contains('安全扫描结果').should('be.visible')
      cy.contains('SUSPICIOUS').should('be.visible')
      cy.contains('检测到可疑外发行为').should('be.visible')
    })

    it('审核通过应调用正确接口', () => {
      mockSkills([
        {
          id: 'test-pending-id',
          name: 'pending-skill',
          description: '待审核技能',
          version: '1.0.0',
          author: 'user',
          enabled: false,
          source: 'admin_upload',
          review_status: 'pending',
          is_builtin: false,
          invoke_count: 0,
          updated_at: now(),
        },
      ])

      cy.intercept('GET', '/api/skills/test-pending-id/scan-results', {
        body: { skill_id: 'test-pending-id', scan_result: null },
      })

      cy.intercept('POST', '/api/skills/test-pending-id/review', (req) => {
        expect(req.body.approved).to.eq(true)
        req.reply({ statusCode: 200, body: { review_status: 'approved' } })
      }).as('reviewSkill')

      cy.visit('/extensions')
      cy.wait('@getSkills')
      cy.contains('tr', 'pending-skill').within(() => {
        cy.contains('button', '审核').click()
      })
      cy.get('[data-slot="dialog-content"]').contains('button', '通过').click()
      cy.wait('@reviewSkill')
    })

    it('插件审核弹窗应加载插件扫描结果', () => {
      cy.intercept('GET', '/api/plugins/plugin-pending/scan-results', {
        body: {
          plugin_id: 'plugin-pending',
          scan_result: {
            scanner_type: 'cisco-ai-skill-scanner',
            verdict: 'SAFE',
            is_safe: true,
            max_severity: 'LOW',
            findings_count: 0,
            findings: [],
            created_at: now(),
          },
        },
      }).as('getPluginScanResults')

      cy.visit('/extensions')
      cy.wait('@getSkills')
      cy.contains('button', '插件管理').click()
      cy.wait('@getPlugins')
      cy.contains('tr', 'pending-plugin').within(() => {
        cy.contains('button', '审核').click()
      })

      cy.wait('@getPluginScanResults')
      cy.contains('审核：pending-plugin').should('be.visible')
      cy.contains('安全扫描结果').should('be.visible')
      cy.contains('SAFE').should('be.visible')
    })
  })

  describe('重扫/下架/启用禁用', () => {
    it('重扫按钮应调用 rescan API', () => {
      cy.intercept('POST', '/api/skills/skill-scan-failed/rescan', {
        statusCode: 200,
        body: { id: 'skill-scan-failed', review_status: 'pending', enabled: false },
      }).as('rescanSkill')

      cy.visit('/extensions')
      cy.wait('@getSkills')
      cy.contains('tr', 'scan-failed-skill').within(() => {
        cy.contains('button', '重扫').click()
      })
      cy.wait('@rescanSkill')
    })

    it('下架按钮应调用 yank API', () => {
      cy.intercept('POST', '/api/skills/skill-approved-upload/yank', {
        statusCode: 200,
        body: { id: 'skill-approved-upload', review_status: 'yanked', enabled: false },
      }).as('yankSkill')

      cy.visit('/extensions')
      cy.wait('@getSkills')
      cy.window().then((win) => {
        cy.stub(win, 'confirm').returns(true)
      })
      cy.contains('tr', 'approved-skill').within(() => {
        cy.contains('button', '下架').click()
      })
      cy.wait('@yankSkill')
    })

    it('插件下架按钮应调用 plugin yank API', () => {
      cy.intercept('POST', '/api/plugins/plugin-approved-upload/yank', {
        statusCode: 200,
        body: { id: 'plugin-approved-upload', review_status: 'yanked', enabled: false },
      }).as('yankPlugin')

      cy.visit('/extensions')
      cy.wait('@getSkills')
      cy.window().then((win) => {
        cy.stub(win, 'confirm').returns(true)
      })

      cy.contains('button', '插件管理').click()
      cy.wait('@getPlugins')
      cy.contains('tr', 'approved-plugin').within(() => {
        cy.contains('button', '下架').click()
      })
      cy.wait('@yankPlugin')
    })

    it('切换开关应调用 disable API', () => {
      cy.intercept('POST', '/api/skills/skill-delegation/disable', {
        statusCode: 200,
        body: { id: 'skill-delegation', enabled: false },
      }).as('disableSkill')

      cy.visit('/extensions')
      cy.wait('@getSkills')
      cy.contains('tr', 'delegation').within(() => {
        cy.get('[data-testid^="toggle-"]').first().click()
      })
      cy.wait('@disableSkill')
    })
  })

  describe('导航', () => {
    it('通过 URL 访问应正常渲染', () => {
      cy.visit('/extensions')
      cy.wait('@getSkills')
      cy.contains('扩展管理').should('be.visible')
      cy.url().should('include', '/extensions')
    })
  })
})
