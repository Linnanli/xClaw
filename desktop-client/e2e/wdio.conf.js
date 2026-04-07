/**
 * WebdriverIO 配置 — IronClaw Desktop E2E 测试
 *
 * 使用 tauri-driver 作为 WebDriver 代理，直接驱动真实 Tauri 应用。
 * 测试的是真实 IPC 调用，不需要 mock。
 *
 * Lima VM 运行说明：
 *   宿主机目录以只读方式挂载到 VM，cargo build 的产物必须输出到 VM 内部可写目录。
 *   通过 CARGO_TARGET_DIR=/tmp/ironclaw-target 重定向编译产物。
 *
 * 运行：
 *   npm test                    # 全部测试（Linux/VM 环境）
 *   npm run test:routines       # 仅定时任务测试
 *   npm run test:lima           # 从 macOS 宿主机通过 Lima 运行
 */

import os from 'os';
import path from 'path';
import { existsSync } from 'fs';
import { spawn, spawnSync } from 'child_process';
import { fileURLToPath } from 'url';

const __dirname = fileURLToPath(new URL('.', import.meta.url));

// 编译产物目录：
// - Lima VM 内运行时，用 /tmp/ironclaw-linux-target 避免与 macOS target/ 冲突
// - 直接在 Linux 上运行时，用 CARGO_TARGET_DIR 或默认 target/
const IS_LINUX = process.platform === 'linux';
const TARGET_DIR = process.env.CARGO_TARGET_DIR
  || (IS_LINUX ? '/tmp/ironclaw-linux-target' : path.resolve(__dirname, '..', 'target'));

const APPLICATION = path.join(TARGET_DIR, 'debug', 'ironclaw-desktop');

let tauriDriver;
let driverExited = false;

function closeTauriDriver() {
  driverExited = true;
  tauriDriver?.kill();
}

['exit', 'SIGINT', 'SIGTERM', 'SIGHUP'].forEach((signal) => {
  process.on(signal, () => {
    closeTauriDriver();
    process.exit();
  });
});

export const config = {
  host: '127.0.0.1',
  port: 4444,

  specs: ['./tests/**/*.spec.js'],
  maxInstances: 1,

  capabilities: [
    {
      maxInstances: 1,
      'tauri:options': {
        application: APPLICATION,
      },
    },
  ],

  reporters: ['spec'],
  framework: 'mocha',
  mochaOpts: {
    ui: 'bdd',
    timeout: 60000,
  },

  onPrepare: () => {
    if (process.platform === 'darwin') {
      console.error([
        '',
        '❌ tauri-driver 不支持 macOS（Apple 未提供 WKWebView WebDriver）',
        '   请使用 Lima VM 运行：npm run test:lima',
        '',
      ].join('\n'));
      process.exit(1);
    }

    console.log(`Building Tauri application...`);
    console.log(`  CARGO_TARGET_DIR: ${TARGET_DIR}`);
    console.log(`  Binary: ${APPLICATION}`);

    // 检查二进制是否已存在（跳过重复编译）
    if (existsSync(APPLICATION) || process.env.SKIP_BUILD === '1') {
      console.log(`Binary ready: ${APPLICATION}`);
    } else {
      const result = spawnSync(
        'cargo',
        ['build', '-p', 'desktop-client'],
        {
          cwd: process.env.SOURCE_ROOT || path.resolve(__dirname, '../..'),
          stdio: 'inherit',
          shell: true,
          env: { ...process.env, CARGO_TARGET_DIR: TARGET_DIR },
        }
      );
      if (result.status !== 0) {
        throw new Error(`cargo build failed with exit code ${result.status}`);
      }
      console.log('Build complete.');
    }
  },

  beforeSession: () => {
    tauriDriver = spawn(
      path.resolve(os.homedir(), '.cargo', 'bin', 'tauri-driver'),
      [],
      { stdio: [null, process.stdout, process.stderr] }
    );

    tauriDriver.on('error', (error) => {
      console.error('tauri-driver error:', error);
      process.exit(1);
    });

    tauriDriver.on('exit', (code) => {
      if (!driverExited) {
        console.error('tauri-driver exited unexpectedly with code:', code);
        process.exit(1);
      }
    });
  },

  afterSession: () => {
    closeTauriDriver();
  },
};
