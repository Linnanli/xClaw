# JSON Schema 参考

本文档定义了 skill-creator 使用的 JSON 结构。

---

## evals.json

定义 skill 的评估用例。保存在 skill 工作目录的 `evals/evals.json`。

```json
{
  "skill_name": "example-skill",
  "evals": [
    {
      "id": 1,
      "prompt": "用户的任务提示",
      "expected_output": "期望结果的描述",
      "files": ["evals/files/sample1.pdf"],
      "expectations": [
        "输出包含 X",
        "使用了脚本 Y"
      ]
    }
  ]
}
```

字段说明：
- `skill_name`：与 skill 文件名匹配
- `evals[].id`：唯一整数标识
- `evals[].prompt`：要执行的任务
- `evals[].expected_output`：人类可读的成功描述
- `evals[].files`：可选的输入文件路径列表（相对于 skill 根目录）
- `evals[].expectations`：可验证的断言列表

---

## grading.json

评分结果。保存在每个测试运行目录中。

```json
{
  "expectations": [
    {
      "text": "输出包含姓名 'John Smith'",
      "passed": true,
      "evidence": "在步骤 3 中找到：'提取的姓名：John Smith, Sarah Johnson'"
    },
    {
      "text": "电子表格在 B10 单元格有 SUM 公式",
      "passed": false,
      "evidence": "没有创建电子表格。输出是一个文本文件。"
    }
  ],
  "summary": {
    "passed": 2,
    "failed": 1,
    "total": 3,
    "pass_rate": 0.67
  },
  "claims": [
    {
      "claim": "表单有 12 个可填字段",
      "type": "factual",
      "verified": true,
      "evidence": "在 field_info.json 中计数了 12 个字段"
    }
  ],
  "eval_feedback": {
    "suggestions": [
      {
        "assertion": "输出包含姓名 'John Smith'",
        "reason": "一个虚构的文档如果提到了这个名字也会通过"
      }
    ],
    "overall": "断言检查了存在性但没有检查正确性。"
  }
}
```

字段说明：
- `expectations[]`：评分后的断言列表
  - `text`：原始断言文本
  - `passed`：布尔值，是否通过
  - `evidence`：支持判定的具体引用或描述
- `summary`：汇总统计
- `claims`：从输出中提取并验证的隐含声明
  - `type`："factual"（事实）、"process"（过程）、"quality"（质量）
- `eval_feedback`：对断言本身的改进建议（仅在有必要时出现）

---

## comparison.json

盲评对比结果。

```json
{
  "winner": "A",
  "reasoning": "输出 A 提供了完整的解决方案，格式正确且包含所有必需字段。输出 B 缺少日期字段且格式不一致。",
  "rubric": {
    "A": {
      "content": {
        "correctness": 5,
        "completeness": 5,
        "accuracy": 4
      },
      "structure": {
        "organization": 4,
        "formatting": 5,
        "usability": 4
      },
      "content_score": 4.7,
      "structure_score": 4.3,
      "overall_score": 9.0
    },
    "B": {
      "content": {
        "correctness": 3,
        "completeness": 2,
        "accuracy": 3
      },
      "structure": {
        "organization": 3,
        "formatting": 2,
        "usability": 3
      },
      "content_score": 2.7,
      "structure_score": 2.7,
      "overall_score": 5.4
    }
  },
  "output_quality": {
    "A": {
      "score": 9,
      "strengths": ["完整的解决方案", "格式良好", "所有字段都存在"],
      "weaknesses": ["标题有轻微的样式不一致"]
    },
    "B": {
      "score": 5,
      "strengths": ["输出可读", "基本结构正确"],
      "weaknesses": ["缺少日期字段", "格式不一致", "数据提取不完整"]
    }
  }
}
```

字段说明：
- `winner`："A"、"B" 或 "TIE"
- `reasoning`：选择赢家的清晰解释
- `rubric`：结构化的量规评估
  - `content`：内容维度评分（正确性、完整性、准确性）
  - `structure`：结构维度评分（组织性、格式、可用性）
  - `content_score`：内容维度平均分（1-5）
  - `structure_score`：结构维度平均分（1-5）
  - `overall_score`：综合分数（1-10）
- `output_quality`：质量摘要

---

## analysis.json

盲评后的分析结果。

```json
{
  "comparison_summary": {
    "winner": "A",
    "winner_skill": "path/to/winner/skill",
    "loser_skill": "path/to/loser/skill",
    "comparator_reasoning": "赢家被选中的原因简述"
  },
  "winner_strengths": [
    "处理多页文档的清晰分步指令",
    "包含了捕获格式错误的验证脚本"
  ],
  "loser_weaknesses": [
    "模糊的指令'适当处理文档'导致了不一致的行为",
    "没有验证脚本，AI 不得不即兴发挥并犯了错误"
  ],
  "improvement_suggestions": [
    {
      "priority": "high",
      "category": "instructions",
      "suggestion": "将'适当处理文档'替换为明确步骤：1) 提取文本，2) 识别章节，3) 按模板格式化",
      "expected_impact": "消除导致不一致行为的模糊性"
    }
  ]
}
```

改进建议的类别：

| 类别 | 描述 |
|------|------|
| `instructions` | 对 skill 文字指令的修改 |
| `tools` | 要添加/修改的脚本、模板或工具 |
| `examples` | 要包含的输入/输出示例 |
| `error_handling` | 处理失败的指导 |
| `structure` | skill 内容的重新组织 |
| `references` | 要添加的外部文档或资源 |
