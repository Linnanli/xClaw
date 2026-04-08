---
name: skill-creator
description: "创建新的 Copilot Skill，改进已有 Skill，并验证 Skill 质量。当用户想要从零创建 skill、把工作流固化成 skill、更新或优化现有 skill、测试 skill 效果时，激活此 skill。即使用户没有明确说"创建 skill"，只要他们在描述一个重复性工作流并希望自动化，也应该考虑激活。Triggers: create skill for, automate workflow, turn process into skill, every day I have to, daily I need to."
---

# Skill Creator

创建 Copilot Skill 的过程是一个迭代循环：

1. 搞清楚 skill 要做什么，大致怎么做
2. 写一份草稿
3. 设计 2-3 个测试用例，用 skill 执行它们
4. 帮用户评估结果（定性 + 定量）
5. 根据反馈改写 skill
6. 重复，直到满意
7. 扩大测试集，更大规模验证

判断用户在这个流程的哪个阶段，然后帮他们推进。如果用户说"不需要跑评估，直接和我聊就行"，就按他们的节奏来。

---

## 创建 Skill

### 捕获意图

先理解用户的意图。如果当前对话已经包含了用户想固化的工作流，先从对话历史提取答案：
- 用了哪些工具、步骤顺序
- 用户做了哪些纠正
- 观察到的输入输出格式

确认以下四点：
1. 这个 skill 要让 AI 做什么？
2. 什么时候应该触发？（什么用户短语/上下文）
3. 期望的输出格式是什么？
4. 是否需要测试用例验证？（有客观可验证输出的 skill 适合测试用例；主观输出通常不需要）

### 边界调研

主动问清楚边界情况、输入输出格式、示例文件、成功标准和依赖。写测试用例之前先搞定这些。

检查项目里是否已有类似 skill（`.github/skills/`），避免重复。

### 写 Skill 文件

**触发描述**（YAML frontmatter 中的 `description` 字段）：
- 说明什么时候触发、做什么
- 包含"什么时候用"的关键词，不放在正文里
- AI 倾向于"不够主动"地使用 skill，所以触发描述要稍微"主动"一点
- 例：不要写"如何构建仪表盘"，而是写"构建数据仪表盘。当用户提到仪表盘、数据可视化、内部指标时都应使用，即使没明确说'仪表盘'"

**Copilot Skill 文件结构：**

```
.github/skills/<skill-name>/
├── SKILL.md           # 必须，名称必须匹配文件夹名
└── references/        # 可选，按需加载的详细文档
```

**SKILL.md frontmatter 格式：**

```yaml
---
name: skill-name              # 必须：1-64 字符，小写字母数字+连字符，必须匹配文件夹名
description: 'What and when to use. Max 1024 chars.'
argument-hint: 'Optional hint shown for slash invocation'
user-invocable: true          # 可选：是否显示为 slash 命令（默认 true）
---
```

### Skill 写作指南

#### 渐进式加载

Skill 使用三层加载：
1. `description` frontmatter（约 100 词）— 始终在上下文中
2. SKILL.md 正文（理想情况下 <500 行）— skill 触发时加载
3. `references/` 目录 — 按需加载，不限大小

正文控制在 500 行以内；接近限制时，把细节移到 `references/` 并在正文中明确指向。

#### 多领域/框架组织

```
cloud-deploy/
├── SKILL.md           # 工作流 + 选择逻辑
└── references/
    ├── aws.md
    ├── gcp.md
    └── azure.md
```

---

## 评估 Skill

### 设计测试用例

好的测试用例：
- 有客观可验证的输出（文件转换、数据提取、代码生成）
- 有明确的成功标准（断言）
- 覆盖正常路径 + 边界情况

### 执行与评分

使用 `references/grading-guide.md` 中的评分流程评估 skill 输出质量。

评分标准：
- **通过**：有明确证据表明断言为真，反映的是真正的任务完成
- **失败**：没有证据，或证据是表面的
- **不确定**：举证责任在断言一方

### 盲评对比（A/B 测试）

对比两个 skill 版本时，使用 `references/comparison-guide.md` 中的盲评流程。

---

## 改进 Skill

根据评估结果，常见改进方向：

1. **触发不够主动** → 在 description 里加更多触发关键词和场景
2. **步骤不够具体** → 把模糊指令改为明确的操作步骤
3. **正文过长** → 把细节移到 `references/`，正文只保留核心指令
4. **断言太弱** → 检查断言是否真的区分"成功"和"失败"

---

## 参考资源

- `references/comparison-guide.md` — A/B 盲评对比流程
- `references/grading-guide.md` — 评分流程和标准
- `references/schemas.md` — evals.json 和 grading.json 的 JSON schema
