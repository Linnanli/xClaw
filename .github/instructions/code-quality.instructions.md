---
description: "Use when writing any code. Enforces anti-patch-code rules: no appended if-branches, no copy-paste logic, no flag-controlled blocks, no commented-out code, no function over 50 lines, no nesting over 3 levels, no Rust unwrap() in production code, no unnecessary clone(). Also enforces prefer-mature-libraries principle."
applyTo: "**/*.rs,**/*.ts,**/*.tsx"
---

# 代码质量规范：禁止补丁式代码

## 什么是补丁式代码

补丁式代码（Patch Code）是指在已有实现上不断追加逻辑，而不是重新设计结构。典型特征：

- 在函数末尾追加 `if special_case { ... }`
- 给函数加新参数只为处理一个边缘情况
- 复制粘贴已有逻辑并微调
- 用全局 flag 控制某段代码是否执行
- 注释掉旧代码而不是删除

## 写代码前必须做的判断

拿到需求后，先问自己：

1. 现有函数/模块的职责是否还清晰？新需求是否超出了它的职责范围？
2. 是否有 2 处以上相似逻辑可以统一？
3. 最简单的实现是什么？能不能不加新参数、不加新分支就解决？
4. **这个功能有没有成熟的库或平台 API 可以直接用？** 如果有，优先用，不要自己实现。

## 优先使用成熟方案，禁止重复造轮子

遇到一个需求，先按以下顺序检查：

1. **项目内已有实现？** 先搜索 `Cargo.toml`、`package.json`、现有代码
2. **框架/平台原生支持？** Tauri、React、Axum 等框架本身是否提供了这个能力
3. **成熟社区库？** crates.io / npm 上是否有被广泛使用的库
4. **以上都没有，才自己实现**

## 触发重构的信号

| 信号 | 正确做法 |
|------|---------|
| 函数超过 50 行 | 拆分为多个单一职责函数 |
| 嵌套超过 3 层 | 提前返回（early return）或提取子函数 |
| 相似逻辑出现 3 次 | 提取为公共函数或 trait |
| 新增参数只服务一个 case | 重新设计函数签名或拆分函数 |
| 需要注释解释"为什么这里要特判" | 这是设计问题，用结构表达意图 |
| 自己实现了一个"轮子" | 检查是否有成熟库，有则替换 |

## 本项目特别注意

- **Rust**：不用 `.clone()` 掩盖所有权设计问题；生产代码禁止 `unwrap()`，用 `?` 或 `expect("reason")` 替代
- **安全模块**（DLP、权限检查）：任何补丁都必须同步更新失败路径测试
- **新增迁移文件**：必须同步更新 `integration_smoke_tests.rs`
- **Tauri 插件**（`tauri-plugin-*`）：涉及文件系统、对话框、通知等能力时**必须优先使用插件**，不要用 Web API 模拟
