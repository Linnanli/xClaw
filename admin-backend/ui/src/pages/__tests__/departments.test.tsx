import { describe, it, expect, beforeEach, vi } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import DepartmentsPage from '@/pages/departments'
import { api } from '@/lib/api'

vi.mock('@/lib/api', () => ({
  api: {
    get: vi.fn(),
    put: vi.fn(),
    post: vi.fn(),
    delete: vi.fn(),
  },
}))

const mockedApi = vi.mocked(api)

function mockDepartmentApi() {
  mockedApi.get.mockImplementation((url: string) => {
    if (url === '/departments') {
      return Promise.resolve({
        data: {
          departments: [
            {
              id: 'dept-1',
              name: '研发部',
              description: '核心研发团队',
              parent_id: null,
              member_count: 3,
              token_quota_enabled: false,
              token_quota_per_day: null,
              created_at: '2026-04-13T00:00:00Z',
              updated_at: '2026-04-13T00:00:00Z',
            },
          ],
        },
      })
    }

    if (url === '/departments/dept-1') {
      return Promise.resolve({
        data: {
          id: 'dept-1',
          name: '研发部',
          description: '核心研发团队',
          parent_id: null,
          member_count: 3,
          token_quota_enabled: false,
          token_quota_per_day: null,
          created_at: '2026-04-13T00:00:00Z',
          updated_at: '2026-04-13T00:00:00Z',
        },
      })
    }

    if (url === '/departments/dept-1/members') {
      return Promise.resolve({ data: { members: [] } })
    }

    if (url === '/departments/dept-1/model-whitelist') {
      return Promise.resolve({ data: { models: [] } })
    }

    if (url === '/departments/dept-1/skill-whitelist') {
      return Promise.resolve({
        data: {
          skill_ids: ['skill-1'],
          skills: [{ skill_id: 'skill-1', name: 'Code Review Expert' }],
        },
      })
    }

    if (url === '/model-configs') {
      return Promise.resolve({
        data: [
          {
            id: 'model-enabled-1',
            model_id: 'qwen3.6-plus-2026-04-02',
            display_name: 'qwen3.6-plus-2026-04-02',
            provider: 'qwen',
            enabled: true,
          },
          {
            id: 'model-disabled-1',
            model_id: 'qwen3.5-omni-plus-2026-03-15',
            display_name: 'qwen3.5-omni-plus-2026-03-15',
            provider: 'qwen',
            enabled: false,
          },
        ],
      })
    }

    if (url === '/departments/dept-1/quota-summary') {
      return Promise.resolve({
        data: {
          today_total_used_cents: 0,
          custom_limit_cents: null,
          children_limit_sum_cents: 0,
        },
      })
    }

    if (url === '/skills') {
      return Promise.resolve({
        data: {
          skills: [
            { id: 'skill-1', name: 'Code Review Expert', enabled: true, review_status: 'approved' },
            { id: 'skill-2', name: 'Code Simplifier', enabled: true, review_status: 'approved' },
            { id: 'skill-3', name: 'Pending Skill', enabled: true, review_status: 'pending' },
          ],
        },
      })
    }

    return Promise.reject(new Error(`Unexpected GET ${url}`))
  })

  mockedApi.put.mockResolvedValue({ data: { ok: true } })
}

describe('DepartmentsPage', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockDepartmentApi()
  })

  it('应展示技能白名单统计与技能标签', async () => {
    render(<DepartmentsPage />)

    await waitFor(() => {
      expect(screen.getByText('可用技能白名单')).toBeInTheDocument()
    })

    expect(screen.getByText('1 个技能')).toBeInTheDocument()
    expect(screen.getByText('Code Review Expert')).toBeInTheDocument()
  })

  it('保存技能白名单时应调用对应接口', async () => {
    render(<DepartmentsPage />)
    const user = userEvent.setup()

    await waitFor(() => {
      expect(screen.getByText('可用技能白名单')).toBeInTheDocument()
    })

    const editButtons = screen.getAllByRole('button', { name: /^编辑$/ })
    await user.click(editButtons[1])

    await waitFor(() => {
      expect(screen.getByText('编辑技能白名单 — 研发部')).toBeInTheDocument()
    })

    await user.click(screen.getByText('Code Simplifier'))
    await user.click(screen.getByRole('button', { name: '保存白名单' }))

    await waitFor(() => {
      expect(mockedApi.put).toHaveBeenCalledWith('/departments/dept-1/skill-whitelist', {
        skill_ids: ['skill-1', 'skill-2'],
      })
    })
  })

  it('模型白名单弹窗应标记禁用模型并展示后端校验错误', async () => {
    mockedApi.put.mockRejectedValueOnce({
      response: {
        data: {
          details: '禁止设置禁用模型: qwen3.5-omni-plus-2026-03-15',
        },
      },
    })

    render(<DepartmentsPage />)
    const user = userEvent.setup()

    await waitFor(() => {
      expect(screen.getByText('可用技能白名单')).toBeInTheDocument()
    })

    const editButtons = screen.getAllByRole('button', { name: /^编辑$/ })
    await user.click(editButtons[0])

    await waitFor(() => {
      expect(screen.getByText('编辑模型白名单 — 研发部')).toBeInTheDocument()
    })

    expect(screen.getByText('已禁用')).toBeInTheDocument()

    await user.click(screen.getByText('qwen3.5-omni-plus-2026-03-15'))
    await user.click(screen.getByRole('button', { name: '保存白名单' }))

    await waitFor(() => {
      expect(screen.getByText('禁止设置禁用模型: qwen3.5-omni-plus-2026-03-15')).toBeInTheDocument()
    })
  })

  it('模型白名单保存失败时应回退展示 error 字段', async () => {
    mockedApi.put.mockRejectedValueOnce({
      response: {
        data: {
          error: '保存失败：模型不可用',
        },
      },
    })

    render(<DepartmentsPage />)
    const user = userEvent.setup()

    await waitFor(() => {
      expect(screen.getByText('可用技能白名单')).toBeInTheDocument()
    })

    const editButtons = screen.getAllByRole('button', { name: /^编辑$/ })
    await user.click(editButtons[0])

    await waitFor(() => {
      expect(screen.getByText('编辑模型白名单 — 研发部')).toBeInTheDocument()
    })

    await user.click(screen.getByRole('button', { name: '保存白名单' }))

    await waitFor(() => {
      expect(screen.getByText('保存失败：模型不可用')).toBeInTheDocument()
    })
  })

  it('存在根部门时创建弹窗不应再提供顶级部门选项', async () => {
    render(<DepartmentsPage />)
    const user = userEvent.setup()

    await waitFor(() => {
      expect(screen.getByText('可用技能白名单')).toBeInTheDocument()
    })

    await user.click(screen.getByRole('button', { name: '新建部门' }))

    await waitFor(() => {
      expect(screen.getByText('创建部门')).toBeInTheDocument()
    })

    expect(screen.queryByRole('option', { name: '无（顶级部门）' })).not.toBeInTheDocument()
    expect(screen.getByText('系统仅允许一个顶级部门，新部门需挂在现有部门下。')).toBeInTheDocument()
  })

  it('存在根部门时新建部门应默认挂在根部门下', async () => {
    mockedApi.post.mockResolvedValueOnce({ data: { ok: true } })

    render(<DepartmentsPage />)
    const user = userEvent.setup()

    await waitFor(() => {
      expect(screen.getByText('可用技能白名单')).toBeInTheDocument()
    })

    await user.click(screen.getByRole('button', { name: '新建部门' }))
    await user.type(screen.getByPlaceholderText('输入部门名称'), '测试子部门')
    await user.click(screen.getByRole('button', { name: '创建' }))

    await waitFor(() => {
      expect(mockedApi.post).toHaveBeenCalledWith('/departments', {
        name: '测试子部门',
        description: null,
        parent_id: 'dept-1',
        token_quota_enabled: false,
        token_quota_per_day: null,
      })
    })
  })
})