// @vitest-environment jsdom

import { act, createElement } from 'react'
import { createRoot } from 'react-dom/client'
import type { ModelContext } from '@assistant-ui/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

type ModelContextRegistration = {
  getModelContext: () => ModelContext
}

const modelContextRegister = vi.fn<(registration: ModelContextRegistration) => () => void>(() =>
  vi.fn()
)

vi.mock('@assistant-ui/react', async () => {
  const actual = await vi.importActual<typeof import('@assistant-ui/react')>('@assistant-ui/react')
  return {
    ...actual,
    useAui: () => ({
      modelContext: () => ({
        register: modelContextRegister
      })
    })
  }
})

import { ModelSelector } from '../components/assistant-ui'

import {
  assistantModelOptions,
  assistantMessage,
  defaultAssistantModelId,
  extractTextFromAppendMessage,
  initialAssistantMessages,
  userMessage
} from './assistantMessages'

describe('assistant-ui message helpers', () => {
  it('extracts only text parts from an assistant-ui append message', () => {
    expect(
      extractTextFromAppendMessage({
        role: 'user',
        content: [
          { type: 'text', text: 'hello' },
          { type: 'image', image: 'data:image/png;base64,a' }
        ],
        attachments: [],
        createdAt: new Date('2026-01-01T00:00:00Z'),
        parentId: null,
        sourceId: null,
        runConfig: undefined,
        metadata: { custom: {} }
      })
    ).toBe('hello')
  })

  it('creates assistant-ui compatible user and assistant messages', () => {
    expect(userMessage('u1', 'hi').role).toBe('user')
    expect(assistantMessage('a1', 'there').status).toEqual({
      type: 'complete',
      reason: 'stop'
    })
  })

  it('starts with an empty thread so the UI can render the welcome composer', () => {
    expect(initialAssistantMessages()).toEqual([])
  })

  it('defines a selectable default assistant model', () => {
    expect(assistantModelOptions.some((model) => model.id === defaultAssistantModelId)).toBe(true)
  })
})

describe('ModelSelector', () => {
  let container: HTMLDivElement
  let root: ReturnType<typeof createRoot>

  beforeEach(() => {
    container = document.createElement('div')
    document.body.appendChild(container)
    root = createRoot(container)
    modelContextRegister.mockClear()
  })

  afterEach(() => {
    act(() => {
      root.unmount()
    })
    container.remove()
  })

  it('registers the selected model in assistant-ui model context', () => {
    act(() => {
      root.render(createElement(ModelSelector, { models: assistantModelOptions }))
    })

    const registration = modelContextRegister.mock.calls.at(-1)?.[0]
    expect(registration?.getModelContext()).toEqual({
      config: {
        modelName: defaultAssistantModelId
      }
    })
  })

  it('registers a controlled model value in assistant-ui model context', () => {
    act(() => {
      root.render(
        createElement(ModelSelector, {
          models: assistantModelOptions,
          value: assistantModelOptions[1].id
        })
      )
    })

    const registration = modelContextRegister.mock.calls.at(-1)?.[0]
    expect(registration?.getModelContext()).toEqual({
      config: {
        modelName: assistantModelOptions[1].id
      }
    })
  })
})
