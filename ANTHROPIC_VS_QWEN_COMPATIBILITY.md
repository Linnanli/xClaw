# Anthropic API 和 Qwen API 兼容性说明

**问题**: Anthropic API 和 Qwen 兼容吗?  
**答案**: ❌ **不兼容** - 它们是完全不同的 API

---

## 为什么不兼容?

### Anthropic API
- **协议**: 专有的 Anthropic 协议
- **认证**: `x-api-key` 头
- **模型**: Claude 系列
- **文档**: https://docs.anthropic.com

### Qwen API
- **协议**: OpenAI 兼容协议
- **认证**: `Authorization: Bearer` 头
- **模型**: Qwen 系列
- **文档**: https://help.aliyun.com/zh/dashscope/

### 关键差异

| 方面 | Anthropic | Qwen |
|------|-----------|------|
| 协议 | 专有 | OpenAI 兼容 |
| 认证方式 | x-api-key | Bearer Token |
| 请求格式 | Anthropic 格式 | OpenAI 格式 |
| 响应格式 | Anthropic 格式 | OpenAI 格式 |
| 模型名称 | claude-* | qwen-* |

---

## 后端如何处理?

后端通过 `ProviderRegistry` 支持多个提供商，每个提供商有自己的处理方式：

### Anthropic 处理流程
```
请求 → Anthropic 协议处理 → Claude API → Anthropic 格式响应 → 返回
```

### Qwen 处理流程
```
请求 → OpenAI 兼容协议处理 → Qwen API → OpenAI 格式响应 → 返回
```

---

## 如何切换?

### 从 Anthropic 切换到 Qwen

**步骤 1**: 停止后端服务

```bash
# 按 Ctrl+C 停止后端
```

**步骤 2**: 修改环境变量

```bash
# 清除 Anthropic 配置
unset ANTHROPIC_API_KEY

# 设置 Qwen 配置
export LLM_BACKEND="openai_compatible"
export LLM_BASE_URL="https://dashscope.aliyuncs.com/compatible-mode/v1"
export LLM_API_KEY="sk-..."  # Qwen API 密钥
export LLM_MODEL="qwen-max"
```

**步骤 3**: 重启后端

```bash
cargo run -- run --cli-only --no-onboard
```

### 从 Qwen 切换到 Anthropic

**步骤 1**: 停止后端服务

```bash
# 按 Ctrl+C 停止后端
```

**步骤 2**: 修改环境变量

```bash
# 清除 Qwen 配置
unset LLM_BACKEND
unset LLM_BASE_URL
unset LLM_API_KEY
unset LLM_MODEL

# 设置 Anthropic 配置
export ANTHROPIC_API_KEY="sk-ant-api..."
```

**步骤 3**: 重启后端

```bash
cargo run -- run --cli-only --no-onboard
```

---

## 为什么后端支持两种?

后端的设计允许用户选择最适合他们的提供商：

1. **Anthropic Claude**
   - 优点: 功能完整，性能优秀
   - 缺点: 需要付费
   - 适合: 需要高质量回复的用户

2. **Qwen**
   - 优点: 中文支持好，价格便宜
   - 缺点: 需要通过 openai_compatible 后端
   - 适合: 中文用户，预算有限的用户

3. **其他提供商**
   - OpenAI, Groq, DeepSeek, Mistral, 等等
   - 每个提供商都有自己的优缺点

---

## 常见问题

**Q: 可以同时使用 Anthropic 和 Qwen 吗?**  
A: 不可以。一次只能配置一个提供商。但可以通过修改环境变量快速切换。

**Q: 如何知道当前使用的是哪个提供商?**  
A: 查看后端启动时的日志。会显示类似:
```
LLM backend: anthropic
Model: claude-sonnet-4-20250514
```

**Q: 如果我想同时支持多个提供商怎么办?**  
A: 这需要修改后端代码，支持在运行时选择提供商。目前不支持。

**Q: Qwen 的 openai_compatible 后端是什么意思?**  
A: 意思是 Qwen 提供了一个与 OpenAI API 兼容的接口。后端通过这个接口调用 Qwen。

**Q: 为什么 Qwen 需要 openai_compatible 后端?**  
A: 因为 Qwen 官方提供的是 OpenAI 兼容的 API，而不是原生的 Qwen API。

---

## 技术细节

### Anthropic 请求格式
```json
{
  "model": "claude-sonnet-4-20250514",
  "max_tokens": 1024,
  "messages": [
    {
      "role": "user",
      "content": "Hello"
    }
  ]
}
```

### Qwen 请求格式 (OpenAI 兼容)
```json
{
  "model": "qwen-max",
  "max_tokens": 1024,
  "messages": [
    {
      "role": "user",
      "content": "Hello"
    }
  ]
}
```

### 认证方式

**Anthropic**:
```
Authorization: Bearer sk-ant-api...
```

**Qwen**:
```
Authorization: Bearer sk-...
```

---

## 总结

- ❌ Anthropic 和 Qwen **不兼容**
- ✅ 后端支持两种提供商
- ✅ 可以通过修改环境变量切换
- ✅ 每个提供商有自己的优缺点
- ✅ 选择最适合你的提供商

