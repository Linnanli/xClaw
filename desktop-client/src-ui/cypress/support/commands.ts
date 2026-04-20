/// <reference types="cypress" />

type CypressTauriMockConfig = {
  commandHandlers?: Record<string, unknown>;
  threadHistoryById?: Record<string, Array<Record<string, unknown>>>;
  threads?: Array<{
    id: string;
    title: string;
    started_at?: string;
    last_activity?: string;
  }>;
};

declare global {
  namespace Cypress {
    interface Chainable {
      installTauriMock(options?: CypressTauriMockConfig): Chainable<void>;
      emitChatEvent(payload: Record<string, unknown>): Chainable<void>;
    }
  }
}

Cypress.Commands.add('installTauriMock', (options: CypressTauriMockConfig = {}) => {
  cy.visit('/app', {
    onBeforeLoad(win) {
      (win as Window & { __XCLAW_CYPRESS_TAURI_MOCK__?: CypressTauriMockConfig }).__XCLAW_CYPRESS_TAURI_MOCK__ = options;
    },
  });
});

Cypress.Commands.add('emitChatEvent', (payload: Record<string, unknown>) => {
  cy.window().then((win) => {
    const emitter = (
      win as Window & {
        __XCLAW_CYPRESS_TAURI_EMIT_CHAT_EVENT__?: (payload: Record<string, unknown>) => void;
      }
    ).__XCLAW_CYPRESS_TAURI_EMIT_CHAT_EVENT__;

    if (!emitter) {
      throw new Error('Cypress Tauri chat-stream emitter is not available');
    }

    emitter(payload);
  });
});

export {};
