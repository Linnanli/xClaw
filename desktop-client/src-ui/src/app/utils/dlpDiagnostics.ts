/**
 * DLP 诊断工具
 * 用于测试和调试 DLP 功能
 */

import { invoke } from '@tauri-apps/api/core';

export interface DlpDiagnosticResult {
  test_name: string;
  input: string;
  expected: string;
  actual: string;
  passed: boolean;
  error?: string;
}

/**
 * 运行 DLP 诊断测试
 */
export async function runDlpDiagnostics(): Promise<DlpDiagnosticResult[]> {
  const results: DlpDiagnosticResult[] = [];
  
  // 测试用例
  const testCases = [
    {
      name: '身份证号检测（110101）',
      input: '我的身份证号是 110101199003071234',
      expected: '110************234',
    },
    {
      name: '身份证号检测（330326）',
      input: '我的身份证号是 330326199408015618',
      expected: '330************618',
    },
    {
      name: '手机号检测',
      input: '联系我：13800138000',
      expected: '138*****000',
    },
    {
      name: 'API密钥阻止',
      input: '阿里云密钥：LTAI4G8aB9cD2eFgH3iJ',
      expected: 'BLOCKED',
    },
    {
      name: '普通文本',
      input: '你好，今天天气怎么样？',
      expected: 'PASS',
    },
  ];
  
  for (const testCase of testCases) {
    try {
      console.log(`🧪 Testing: ${testCase.name}`);
      console.log(`   Input: ${testCase.input}`);
      
      const result = await invoke('scan_user_input', { content: testCase.input });
      
      console.log(`   Result:`, result);
      
      const scanResult = result as any;
      
      let passed = false;
      let actual = '';
      
      if (testCase.expected === 'BLOCKED') {
        // 期望被阻止
        passed = scanResult.was_blocked === true;
        actual = scanResult.was_blocked ? 'BLOCKED' : 'NOT_BLOCKED';
      } else if (testCase.expected === 'PASS') {
        // 期望通过
        passed = scanResult.had_sensitive_data === false;
        actual = scanResult.had_sensitive_data ? 'DETECTED' : 'PASS';
      } else {
        // 期望脱敏
        passed = scanResult.sanitized_content.includes(testCase.expected);
        actual = scanResult.sanitized_content;
      }
      
      results.push({
        test_name: testCase.name,
        input: testCase.input,
        expected: testCase.expected,
        actual,
        passed,
      });
      
      console.log(`   ${passed ? '✅ PASS' : '❌ FAIL'}`);
    } catch (error) {
      console.error(`   ❌ ERROR:`, error);
      results.push({
        test_name: testCase.name,
        input: testCase.input,
        expected: testCase.expected,
        actual: 'ERROR',
        passed: false,
        error: error instanceof Error ? error.message : String(error),
      });
    }
  }
  
  return results;
}

/**
 * 打印诊断结果
 */
export function printDlpDiagnostics(results: DlpDiagnosticResult[]): void {
  console.log('\n📊 DLP 诊断结果:');
  console.log('═'.repeat(80));
  
  let passedCount = 0;
  let failedCount = 0;
  
  for (const result of results) {
    if (result.passed) {
      passedCount++;
      console.log(`✅ ${result.test_name}`);
    } else {
      failedCount++;
      console.log(`❌ ${result.test_name}`);
      console.log(`   输入: ${result.input}`);
      console.log(`   期望: ${result.expected}`);
      console.log(`   实际: ${result.actual}`);
      if (result.error) {
        console.log(`   错误: ${result.error}`);
      }
    }
  }
  
  console.log('═'.repeat(80));
  console.log(`总计: ${results.length} 个测试`);
  console.log(`通过: ${passedCount} 个`);
  console.log(`失败: ${failedCount} 个`);
  console.log(`成功率: ${((passedCount / results.length) * 100).toFixed(1)}%`);
}

/**
 * 在浏览器控制台中运行诊断
 * 使用方法：在浏览器控制台中输入 window.runDlpDiagnostics()
 */
if (typeof window !== 'undefined') {
  (window as any).runDlpDiagnostics = async () => {
    const results = await runDlpDiagnostics();
    printDlpDiagnostics(results);
    return results;
  };
}
