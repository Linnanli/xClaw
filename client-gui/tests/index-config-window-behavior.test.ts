import { describe, it, expect } from 'vitest';
import path from 'node:path';
import fs from 'node:fs';

const indexPath = path.resolve(process.cwd(), 'src/main/index.ts');

describe('Main process window/config behavior', () => {
  it('second-instance path focuses existing window and only recreates when none found', () => {
    const source = fs.readFileSync(indexPath, 'utf8');
    const secondInstanceBlock =
      source.match(/app\.on\('second-instance'[\s\S]*?\n  }\);\n}/)?.[0] || '';

    expect(secondInstanceBlock).toContain('BrowserWindow.getAllWindows()');
    expect(secondInstanceBlock).toContain('focused existing window');
    // createWindow is allowed as a fallback when no existing window is found
    expect(secondInstanceBlock).toContain('No existing window found');
  });

  it('keeps credential guard on the legacy session-manager path only', () => {
    const source = fs.readFileSync(indexPath, 'utf8');
    const bridgePathIndex = source.indexOf("if (runnerDecision.runner === 'dasclaw')");
    const credentialGuardIndex = source.indexOf(
      "event.type === 'session.start' && !configStore.hasUsableCredentialsForActiveSet()"
    );
    const sessionStartGuard =
      source.match(
        /if \(event\.type === 'session\.start' && !configStore\.hasUsableCredentialsForActiveSet\(\)\) \{[\s\S]*?return null;\n  \}/
      )?.[0] || '';

    expect(bridgePathIndex).toBeGreaterThanOrEqual(0);
    expect(credentialGuardIndex).toBeGreaterThanOrEqual(0);
    expect(bridgePathIndex).toBeLessThan(credentialGuardIndex);
    expect(sessionStartGuard).toContain('hasUsableCredentialsForActiveSet');
    expect(sessionStartGuard).toContain("code: 'CONFIG_REQUIRED_ACTIVE_SET'");
    expect(sessionStartGuard).toContain("action: 'open_api_settings'");
    expect(sessionStartGuard).not.toContain("type: 'config.status'");
  });

  it('uses dasclaw app-server by default unless the runner explicitly selects legacy', () => {
    const source = fs.readFileSync(indexPath, 'utf8');
    const runnerDecisionResolver =
      source.match(/function resolveAgentRunnerDecision\([\s\S]*?\n}/)?.[0] || '';
    const handleClientEventBlock =
      source.match(/async function handleClientEvent\([\s\S]*?const sm = sessionManager!;/)?.[0] ||
      '';

    expect(source).not.toContain('function shouldUseDasclawAppServerBridge');
    expect(handleClientEventBlock).toContain('const runnerDecision = resolveAgentRunnerDecision');
    expect(handleClientEventBlock).toContain('process.env.OPEN_COWORK_AGENT_RUNNER');
    expect(handleClientEventBlock).toContain("runnerDecision.runner === 'dasclaw'");
    expect(runnerDecisionResolver).toContain("runner === 'dasclaw'");
    expect(runnerDecisionResolver).toContain("runner === 'legacy'");
    expect(runnerDecisionResolver).toContain("runner === 'session-manager'");
    expect(runnerDecisionResolver).toContain("reason: 'explicit-dev-opt-in'");
    expect(runnerDecisionResolver).not.toContain('toLowerCase');
    expect(runnerDecisionResolver).not.toContain("runner.trim() === 'legacy'");
    expect(runnerDecisionResolver).toContain('Unknown OPEN_COWORK_AGENT_RUNNER');
    expect(runnerDecisionResolver).toContain('using dasclaw app-server path');
    expect(runnerDecisionResolver).toContain("reason: 'unknown-fail-closed'");
    expect(runnerDecisionResolver).not.toContain("return { runner: 'legacy' }");
    expect(handleClientEventBlock).not.toContain("OPEN_COWORK_AGENT_RUNNER === 'dasclaw'");
  });
});
