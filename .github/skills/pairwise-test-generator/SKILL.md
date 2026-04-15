---
name: pairwise-test-generator
description: "生成 pairwise/PICT 测试资产并落到现有测试族。用户提到 pairwise、PICT、组合测试、测试矩阵、从需求生成测试代码、把 PICT 结果转成 Rust/TypeScript 测试时应使用。先用 pict-test-designer 做参数/取值/约束设计，再按需要调用 scripts/pict_generate.py 生成真实组合，最后映射到现有单元/失败路径/契约/安全审计测试，而不是新建平行测试。"
argument-hint: "需求描述、目标代码路径，或已有的 PICT model 文件路径"
---

# Pairwise Test Generator

这个 skill 用来把“组合测试设计”推进到“可执行测试资产”。

它不是替代 `pict-test-designer`，而是把下面这条链路一次做完：

1. 分析需求或代码
2. 用 `pict-test-designer` 设计参数、取值、约束、预期结果
3. 判断是否需要真实 PICT 生成
4. 如需要，调用 [scripts/pict_generate.py](scripts/pict_generate.py)
5. 把结果映射到现有测试文件和测试层级
6. 生成或更新测试代码骨架

## 何时使用

在这些场景使用：

- 用户要“直接生成 PICT 测试代码”
- 用户不想手动切换 PICT 工具
- 用户已经有 PICT model，想直接生成测试矩阵或测试代码
- 用户要把组合矩阵映射到 Rust/TypeScript 测试
- 用户要在 agent 内完成“设计 -> 生成 -> 落代码”整条流程

在这些场景不要使用：

- 只是想理解 PICT 或 pairwise 概念
- 只是要一份测试设计方案，不需要真实生成或代码落地
- 目标是普通单参数/小函数测试，不需要组合设计

## 工作流

### 1. 先判断输入类型

输入通常有三类：

1. 需求描述
2. 代码路径或模块名
3. 已有 PICT model 文件

如果是 1 或 2：
- 先按 `pict-test-designer` 输出参数、取值、约束、预期结果

如果是 3：
- 直接进入真实生成阶段

### 2. 判断是否需要真实 PICT 生成

只在这些条件满足时调用真实生成：

- 参数较多，手工 pairwise 审查容易漏
- 用户明确要求真实 pairwise 组合
- 需要可复现组合结果用于 CI/评审
- 这是审批、权限、配额、策略、安全等高风险组合逻辑

否则：
- 停在设计矩阵
- 直接把设计矩阵交给模型生成测试代码

### 3. 真实生成命令

统一通过 [scripts/pict_generate.py](scripts/pict_generate.py) 调用，不要手动切换 `pict`、`pypict`、Docker：

```bash
python3 scripts/pict_generate.py <model-file> --format json
```

如果当前环境没有本机后端但允许 Docker：

```bash
python3 scripts/pict_generate.py <model-file> --backend docker --docker-build-if-missing --format json
```

### 4. 代码落地规则

- 只扩充现有测试文件和测试族
- 不新建一套平行的“大而全测试”
- 组合用例必须按现有分层拆进：
  - 单元测试
  - 失败路径测试
  - 契约测试
  - 安全审计测试
  - 集成测试
  - E2E 测试

- 若属于 Tauri 命令，记得补命令契约测试
- 若涉及启动状态或异步状态注入，记得补 `engine_startup_tests`
- 若涉及迁移或新表，不要只停留在组合测试，还要更新 smoke tests

## 输出要求

输出至少包含：

1. Generation Mode
   - `design-only`
   - `real-pict-generated`

2. PICT Model

3. 组合矩阵

4. 预期结果说明

5. 测试映射计划
   - 哪些 case 去哪个现有测试文件

6. 如果用户要求落代码：
   - 直接修改现有测试文件
   - 跑相关测试或编译验证

## 参考

- [pict-test-designer](../pict-test-designer/SKILL.md)
- [scripts/pict_generate.py](scripts/pict_generate.py)
- [docs/testing-guide.md](docs/testing-guide.md)
