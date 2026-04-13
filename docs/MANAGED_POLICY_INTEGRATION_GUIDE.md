# Managed Policy 联调手册

本手册用于 x-claw 的签名受管策略（Managed Policy）端到端联调。

## 适用范围

本手册覆盖以下链路：

1. admin-backend 提供 `GET /api/client-policy`
2. 响应 envelope 字段完整且格式正确（`algorithm`、`key_id`、`manifest_payload`、`signature`）
3. `policy_version` 单调递增
4. 冒烟脚本退出码为 `0`

## 前置条件

1. PostgreSQL 容器已启动（`admin-backend-postgres`）
2. admin-backend 可访问（`http://127.0.0.1:3000`）
3. 已安装 `curl` 和 `jq`
4. 使用最新脚本：`scripts/managed-policy-smoke.sh`

## 上传技能包格式自检（避免 400）

当调用 `/api/skills/upload-package` 返回 400 且提示 frontmatter 缺少字段时，通常是包内 `SKILL.md` 的 YAML frontmatter 不完整。

后端当前会强制校验这些必填项：

1. `name`
2. `version`
3. `description`
4. `activation`（且 `activation.keywords` 或 `activation.patterns` 至少一个非空）

最小可用 `SKILL.md` 示例：

```md
---
name: demo-skill
version: 1.0.0
description: demo skill for upload-package test
activation:
  keywords:
    - demo
---

# Demo Skill

This is a demo skill.
```

打包要求：

1. 包格式仅支持 `.zip`、`.tar.gz`、`.tgz`
2. 包内必须存在文件名严格为 `SKILL.md` 的文件（可在任意子目录）
3. `SKILL.md` 必须是 UTF-8 文本，且不超过 64 KiB

如果你收到错误：`frontmatter 缺少必填字段 version`，优先检查：

1. 是否把 `version` 写在 frontmatter 外部
2. frontmatter 是否正确使用 `---` 开始和结束
3. 字段名是否拼写为 `version`（而不是 `ver`、`v` 等）

## 快速开始（可直接复制）

### 1）启动带签名密钥的 admin-backend

```bash
# 在仓库根目录执行
export MANAGED_POLICY_SIGNING_KEY_B64="$(openssl rand -base64 32)"
cd admin-backend
cargo run
```

保持该终端运行，在新终端继续下一步。

### 2）创建临时用户并获取 JWT

```bash
TS=$(date +%s)
USER="smoke_${TS}"
EMAIL="smoke_${TS}@example.com"
PASS='SmokePass!123'

curl -sS -X POST http://127.0.0.1:3000/api/auth/register \
  -H 'Content-Type: application/json' \
  -d "{\"username\":\"${USER}\",\"email\":\"${EMAIL}\",\"password\":\"${PASS}\"}" >/dev/null

TOKEN=$(curl -sS -X POST http://127.0.0.1:3000/api/auth/login \
  -H 'Content-Type: application/json' \
  -d "{\"username\":\"${USER}\",\"password\":\"${PASS}\"}" | jq -r '.access_token')

echo "token_len=${#TOKEN}"
```

### 3）执行冒烟脚本

```bash
CLIENT_TOKEN="$TOKEN" ./scripts/managed-policy-smoke.sh
```

预期输出应包含：

1. `[OK] Envelope fields are valid`
2. `[OK] policy_version monotonic: ...`
3. `[DONE] Managed policy smoke check passed`

并且退出码应为 `0`。

## 验收清单

1. `/api/client-policy` 返回 HTTP `200`
2. `algorithm` 等于 `ed25519`
3. `key_id` 非空
4. `manifest_payload` 反序列化后包含：
   - `policy_version`
   - `issued_at`
   - `expires_at`
   - `managed_mode`
   - `allowed_skills`
   - `allowed_extensions`
5. 第二次拉取到的 `policy_version` 大于第一次

## 常见问题排查

### `/api/client-policy` 返回 401 Unauthorized

原因：

- token 无效或已过期

处理：

1. 重新执行注册和登录，获取新的 JWT
2. 将新 token 传给 `CLIENT_TOKEN`

### `/api/client-policy` 返回 404 Not Found

原因：

- 3000 端口上是旧进程或错误服务

处理：

1. 先停止 `:3000` 上旧进程
2. 从当前工作区代码重新启动 `admin-backend`

### `/api/client-policy` 返回 500（签名密钥缺失）

原因：

- 未配置签名私钥环境变量

处理：

1. 导出 `MANAGED_POLICY_SIGNING_KEY_B64`
2. 重启 `admin-backend`

### 脚本看似成功但退出码非 0

原因：

- 使用了旧版脚本，`set -u` 下 trap 清理不安全

处理：

1. 更新为最新 `scripts/managed-policy-smoke.sh`
2. 确认脚本使用 `${VAR:-}` 的安全清理方式

## 可选：Desktop 侧进一步验证

冒烟通过后可启动 desktop-client，并观察日志：

1. 策略来源（`remote` 或 `cache`）
2. `policy_version`
3. 收到非递增版本时的 replay-protection 拒绝日志

## 关联文件

1. `scripts/managed-policy-smoke.sh`
2. `scripts/managed-policy.env.example`
3. `desktop-client/src/managed_policy.rs`
4. `desktop-client/src/engine.rs`
5. `admin-backend/src/routes.rs`
