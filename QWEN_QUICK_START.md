# Qwen 快速启动指南

**状态**: ✅ Qwen 已成功配置并工作

---

## 快速启动（3 步）

### 步骤 1: 启动后端

在终端 1 中运行：

```bash
export LLM_API_KEY="sk-d162b1775e514757b083a2dfe729d6e6"
export LLM_BACKEND="openai_compatible"
export LLM_BASE_URL="https://dashscope.aliyuncs.com/compatible-mode/v1"
export LLM_MODEL="qwen-max"
nohup cargo run -- run --no-onboard < /dev/null > /tmp/backend.log 2>&1 &
```

等待 15-20 秒后端启动。

### 步骤 2: 启动前端

在终端 2 中运行：

```bash
cd desktop-client/src-ui
npm run dev
```

### 步骤 3: 打开浏览器

访问 http://localhost:5173

---

## 验证配置

### 检查后端

```bash
curl http://localhost:3000/api/health
```

预期输出：
```json
{"status":"healthy","channel":"gateway"}
```

### 检查后端日志

```bash
tail -20 /tmp/backend.log | grep -E "model|gateway"
```

预期输出：
```
model     qwen-max  via openai_compatible
gateway   http://127.0.0.1:3000/?token=...
```

### 运行 E2E 测试

```bash
cd desktop-client
cargo test --test qwen_e2e_test diagnose_qwen_setup -- --nocapture --ignored
```

预期结果：
```
✅ 诊断完成 - 所有检查通过!
```

---

## 前端测试

1. 打开 http://localhost:5173
2. 输入消息（例如："你好，Qwen！")
3. 点击发送
4. 应该能收到 Qwen 的中文回复

---

## 常见问题

### Q: 后端启动失败

**A**: 检查是否有旧进程在运行：
```bash
pkill -f "cargo run"
rm -f ~/.ironclaw/ironclaw.pid
```

### Q: 前端无法连接到后端

**A**: 检查令牌是否正确：
- 文件: `desktop-client/src-ui/src/app/components/tabs/ChatTabWithAiSdk.tsx`
- 令牌: `d397b61ad5584603d5691f03e73a0a3eea6fd66d1590ddc64a6b0292c7a2f270`

### Q: 消息发送后没有响应

**A**: 可能是以下原因：
1. 后端没有正确启动（检查日志）
2. Qwen API 密钥无效
3. 网络连接问题

查看后端日志：
```bash
tail -100 /tmp/backend.log
```

### Q: 如何获取新的认证令牌？

**A**: 每次启动后端时都会生成新的令牌。查看后端日志：
```bash
grep "gateway" /tmp/backend.log | tail -1
```

---

## 环境变量说明

| 变量 | 值 | 说明 |
|------|-----|------|
| `LLM_API_KEY` | `sk-d162b1775e514757b083a2dfe729d6e6` | Qwen API 密钥 |
| `LLM_BACKEND` | `openai_compatible` | LLM 后端类型 |
| `LLM_BASE_URL` | `https://dashscope.aliyuncs.com/compatible-mode/v1` | Qwen API 基础 URL |
| `LLM_MODEL` | `qwen-max` | 使用的模型 |

---

## 文件清单

### 已更新的文件

- ✅ `desktop-client/src-ui/src/app/components/tabs/ChatTabWithAiSdk.tsx` - 更新了认证令牌
- ✅ `desktop-client/tests/qwen_e2e_test.rs` - 更新了认证令牌

### 诊断文件

- 📄 `QWEN_DIAGNOSTIC_REPORT.md` - 完整的诊断报告
- 📄 `QWEN_QUICK_START.md` - 本文件

---

## 下一步

1. ✅ 启动后端
2. ✅ 启动前端
3. ✅ 测试聊天功能
4. ✅ 验证 Qwen 响应

---

## 总结

**Qwen 已成功配置！** 

- ✅ 后端能正确调用 Qwen API
- ✅ Qwen 能生成中文响应
- ✅ 前端已更新令牌
- ✅ 可以开始测试了

