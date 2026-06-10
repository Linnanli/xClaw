import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

function readPackageJson() {
  return JSON.parse(readFileSync(resolve(__dirname, '../package.json'), 'utf-8')) as {
    scripts?: Record<string, string>;
  };
}

describe('dev script guard', () => {
  it('keeps npm run dev free from bundled python preparation', () => {
    const packageJson = readPackageJson();

    expect(packageJson.scripts?.dev).toBeDefined();
    expect(packageJson.scripts?.dev).not.toContain('prepare:python');
    expect(packageJson.scripts?.['dev:with-python']).toContain('prepare:python');
  });

  it('keeps dasclaw app-server smoke wired to the standalone script', () => {
    const packageJson = readPackageJson();

    expect(packageJson.scripts?.['smoke:dasclaw-app-server']).toBe(
      'node scripts/dasclaw-app-server-smoke.mjs'
    );
  });

  it('keeps dasclaw app-server smoke focused on the protocol bridge path', () => {
    const smokeScript = readFileSync(
      resolve(__dirname, '../scripts/dasclaw-app-server-smoke.mjs'),
      'utf-8'
    );

    expect(smokeScript).toContain("OPEN_COWORK_AGENT_RUNNER: 'dasclaw'");
    expect(smokeScript).toContain("DASCLAW_APP_SERVER_RUNTIME: 'echo'");
    expect(smokeScript).toContain('DASCLAW_SMOKE_SKIP_BUILD');
    expect(smokeScript).toContain('--user-data-dir=');
    expect(smokeScript).toContain('window.__getNavStatus');
    expect(smokeScript).toContain('result.partialCount !== 1');
    expect(smokeScript).toContain('CDP request timed out');
  });
});
