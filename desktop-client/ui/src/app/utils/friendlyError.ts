/**
 * 将 ironclaw 引擎的原始错误字符串转换为用户友好的中文提示。
 *
 * ironclaw agent loop 在 handle_message 失败时，通过 channel.respond 发送
 * `"Error: {error_chain}"` 格式的普通 response。此模块负责解析这些错误
 * 并转换为人类可读的消息。
 *
 * 错误格式示例：
 * `"Error: LLM error: Provider qwen-max request failed: HttpError: Invalid status code 403 Forbidden with message: {...json...}"`
 */

/** 检测消息内容是否为 ironclaw 错误响应 */
export function isErrorResponse(content: string): boolean {
  return content.startsWith('Error: ');
}

/** 将原始错误字符串转换为友好消息。如果无法识别，返回通用提示。 */
export function friendlyErrorMessage(raw: string): string {
  // 去掉 "Error: " 前缀
  const inner = raw.startsWith('Error: ') ? raw.slice(7) : raw;

  const llm = parseLlmError(inner);
  if (llm) return llm;

  return 'AI 服务出现错误，请稍后重试';
}

/**
 * 解析 LLM 错误并返回友好消息。
 *
 * 格式：`LLM error: Provider {provider} request failed: {reason}`
 * reason 格式：`HttpError: Invalid status code {code} {text} with message: {body}`
 */
function parseLlmError(inner: string): string | null {
  const afterLlm = stripPrefix(inner, 'LLM error: Provider ');
  if (!afterLlm) return null;

  const sepIdx = afterLlm.indexOf(' request failed: ');
  if (sepIdx < 0) return null;

  const provider = afterLlm.slice(0, sepIdx);
  const reason = afterLlm.slice(sepIdx + ' request failed: '.length);

  const code = extractHttpStatus(reason);
  if (code == null) return `模型 ${provider} 请求失败，请检查网络连接`;

  switch (code) {
    case 403: {
      const errorType = extractJsonErrorType(reason);
      if (errorType === 'AllocationQuota.FreeTierOnly') {
        return `模型 ${provider} 免费额度已用完，请前往控制台关闭「仅使用免费额度」选项`;
      }
      return `模型 ${provider} 访问被拒绝（403），请检查 API Key 权限`;
    }
    case 401:
      return `模型 ${provider} 认证失败，请检查 API Key 是否正确`;
    case 429:
      return `模型 ${provider} 请求频率超限，请稍后重试`;
    case 500:
    case 502:
    case 503:
      return `模型 ${provider} 服务暂时不可用，请稍后重试`;
    default:
      return `模型 ${provider} 请求失败（HTTP ${code}）`;
  }
}

/**
 * 从 reason 中提取 HTTP 状态码。
 *
 * 支持两种格式：
 * - `HttpError: Invalid status code 403 Forbidden with message: ...`
 * - `HTTP 403: ...`（旧格式兼容）
 */
function extractHttpStatus(reason: string): number | null {
  // 格式 1：HttpError: Invalid status code {code} ...
  const httpErrorPrefix = 'HttpError: Invalid status code ';
  if (reason.startsWith(httpErrorPrefix)) {
    const afterPrefix = reason.slice(httpErrorPrefix.length);
    const codeStr = afterPrefix.split(/\s/)[0];
    const code = parseInt(codeStr, 10);
    return isNaN(code) ? null : code;
  }
  // 格式 2：HTTP {code}: ...
  const httpPrefix = 'HTTP ';
  if (reason.startsWith(httpPrefix)) {
    const afterPrefix = reason.slice(httpPrefix.length);
    const codeStr = afterPrefix.split(':')[0]?.trim();
    const code = parseInt(codeStr ?? '', 10);
    return isNaN(code) ? null : code;
  }
  return null;
}

/** 从错误字符串中提取 JSON 响应体里的 error.type 字段 */
function extractJsonErrorType(reason: string): string | null {
  const jsonStart = reason.indexOf('{');
  if (jsonStart < 0) return null;
  try {
    const parsed = JSON.parse(reason.slice(jsonStart));
    return parsed?.error?.type ?? null;
  } catch {
    return null;
  }
}

function stripPrefix(s: string, prefix: string): string | null {
  return s.startsWith(prefix) ? s.slice(prefix.length) : null;
}
