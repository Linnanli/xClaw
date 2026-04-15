# Pairwise Test Generator Workflow

## Fast Path

适用于中小规模组合：

1. 分析需求或代码
2. 输出参数、取值、约束、预期结果
3. 直接生成测试代码骨架
4. 映射到现有测试文件

## Full Path

适用于高风险或高复杂度组合：

1. 分析需求或代码
2. 产出 PICT model
3. 调用统一脚本生成真实组合：

```bash
python3 scripts/pict_generate.py model.txt --format json
```

4. 用输出结果生成测试代码
5. 补编译与测试验证

## Recommended Output Shape

```markdown
## Generation Mode
- Mode: real-pict-generated
- Backend: pict | pypict | docker

## PICT Model
...

## Generated Cases
...

## Mapping
- [existing test file A] -> cases 1, 2, 5
- [existing test file B] -> cases 3, 4
```

## Guardrails

- 不把脚本输出原样堆进一个超大测试文件
- 不把 pairwise 组合当成完整测试策略
- 失败路径、安全审计、真实环境验证仍需单独保留