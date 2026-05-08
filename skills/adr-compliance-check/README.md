# adr-compliance-check

x-claw 仓库专用 skill：检测当前 Git 改动是否违反 ADR（Architecture Decision Records）层面的硬性约束。

补 `code-quality-audit` / `code-review-expert` 看不到的盲区——架构纪律。

## 与其他 skill 的差异

| Skill | 关注点 | 触发时机 |
|-------|--------|---------|
| `code-quality-audit` | 代码工艺（长函数 / 重复 / 资源管理） | 写完非平凡实现后 |
| `code-simplifier` | 简化收敛（消嵌套 / 去重 / 命名） | quality-audit 之后；**verbatim port 必须跳过** |
| `adr-compliance-check`（本 skill） | ADR 红线（verbatim 纯度 / drift guard / `.ironclaw` 字面量 / starlark pin / hook 引擎统一 / PR 体例） | PR push 前；触及 `crates/dasclaw_*` / `.github/workflows/` / `scripts/check_codex_*_drift.py` 的任何 PR |
| `code-review-expert` | 综合 review（SOLID / 安全 / 性能） | PR self-review |

## 用法

按需在子 agent 里调用：

```
runSubagent(agentName="adr-compliance-check", prompt="Review current git changes against ADR redlines.")
```

或参考 `SKILL.md` 自行执行 10 步 workflow + §11 输出模板。

## 维护

- 新增 ADR 后：在 `SKILL.md` §1 "收集变更上下文" 与 §11 "输出格式" 里登记
- 新增 verbatim port crate 后：在 `AGENTS.md` 与本 skill 的 drift guard 列表里登记
- 调整红线时：同步改 `AGENTS.md` §"不要碰的红线"
