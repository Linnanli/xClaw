# 路径别名配置

## 概述

为了避免使用冗长的相对路径(如 `../../config/api`),项目配置了路径别名,可以使用简洁的绝对路径导入。

## 可用的别名

| 别名 | 指向目录 | 用途 |
|------|---------|------|
| `@/*` | `./src/*` | 根目录 |
| `@app/*` | `./src/app/*` | App 目录 |
| `@config/*` | `./src/app/config/*` | 配置文件 |
| `@components/*` | `./src/app/components/*` | React 组件 |
| `@hooks/*` | `./src/app/hooks/*` | React Hooks |
| `@utils/*` | `./src/app/utils/*` | 工具函数 |
| `@contexts/*` | `./src/app/contexts/*` | React Contexts |
| `@services/*` | `./src/app/services/*` | 服务层 |

## 使用示例

### 导入配置

```typescript
// ❌ 不推荐 - 使用相对路径
import { API_BASE_URL } from '../../config/api';

// ✅ 推荐 - 使用别名
import { API_BASE_URL } from '@config/api';
```

### 导入组件

```typescript
// ❌ 不推荐
import { Button } from '../../../components/ui/Button';

// ✅ 推荐
import { Button } from '@components/ui/Button';
```

### 导入 Hooks

```typescript
// ❌ 不推荐
import { useAiChat } from '../hooks/useAiChat';

// ✅ 推荐
import { useAiChat } from '@hooks/useAiChat';
```

### 导入工具函数

```typescript
// ❌ 不推荐
import { createSseClient } from '../../utils/sse';

// ✅ 推荐
import { createSseClient } from '@utils/sse';
```

### 导入 Context

```typescript
// ❌ 不推荐
import { useTheme } from '../../contexts/ThemeContext';

// ✅ 推荐
import { useTheme } from '@contexts/ThemeContext';
```

## 配置文件

### Vite 配置 (`vite.config.ts`)

```typescript
export default defineConfig({
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
      '@app': path.resolve(__dirname, './src/app'),
      '@config': path.resolve(__dirname, './src/app/config'),
      '@components': path.resolve(__dirname, './src/app/components'),
      '@hooks': path.resolve(__dirname, './src/app/hooks'),
      '@utils': path.resolve(__dirname, './src/app/utils'),
      '@contexts': path.resolve(__dirname, './src/app/contexts'),
      '@services': path.resolve(__dirname, './src/app/services'),
    },
  },
});
```

### TypeScript 配置 (`tsconfig.json`)

```json
{
  "compilerOptions": {
    "baseUrl": ".",
    "paths": {
      "@/*": ["./src/*"],
      "@app/*": ["./src/app/*"],
      "@config/*": ["./src/app/config/*"],
      "@components/*": ["./src/app/components/*"],
      "@hooks/*": ["./src/app/hooks/*"],
      "@utils/*": ["./src/app/utils/*"],
      "@contexts/*": ["./src/app/contexts/*"],
      "@services/*": ["./src/app/services/*"]
    }
  }
}
```

## 优点

1. **更简洁**: 不需要计算相对路径层级
2. **更清晰**: 一眼就能看出导入的模块类型
3. **更易维护**: 移动文件时不需要更新导入路径
4. **IDE 支持**: TypeScript 配置提供自动补全和跳转

## 注意事项

1. **一致性**: 项目中应该统一使用别名,避免混用相对路径和别名
2. **同级导入**: 同一目录下的文件可以使用相对路径 `./`
3. **父级导入**: 导入父级目录的文件使用 `../`,但跨多级时使用别名

## 迁移指南

如果你有使用相对路径的旧代码,可以按照以下规则迁移:

```typescript
// 规则 1: 导入 config
'../../config/api' → '@config/api'
'../../../config/api' → '@config/api'

// 规则 2: 导入 hooks
'../hooks/useAiChat' → '@hooks/useAiChat'
'../../hooks/useAiChat' → '@hooks/useAiChat'

// 规则 3: 导入 utils
'../../utils/sse' → '@utils/sse'
'../../../utils/sse' → '@utils/sse'

// 规则 4: 导入 contexts
'../../contexts/ThemeContext' → '@contexts/ThemeContext'

// 规则 5: 导入 components
'../common/Button' → '@components/common/Button'
'../../components/ui/Input' → '@components/ui/Input'
```

## IDE 配置

### VS Code

VS Code 会自动识别 `tsconfig.json` 中的路径映射,提供:
- 自动补全
- 跳转到定义
- 重命名重构

### WebStorm

WebStorm 也会自动识别路径映射,无需额外配置。

## 常见问题

### Q: 为什么导入时没有自动补全?

A: 确保:
1. `tsconfig.json` 文件存在且配置正确
2. IDE 已经重新加载配置
3. TypeScript 语言服务正在运行

### Q: 可以添加新的别名吗?

A: 可以,但需要同时更新:
1. `vite.config.ts` 中的 `resolve.alias`
2. `tsconfig.json` 中的 `compilerOptions.paths`

### Q: 别名在测试中可用吗?

A: 可以,Vitest 会自动识别 Vite 配置中的别名。

## 参考资料

- [Vite 路径别名文档](https://vitejs.dev/config/shared-options.html#resolve-alias)
- [TypeScript 路径映射文档](https://www.typescriptlang.org/docs/handbook/module-resolution.html#path-mapping)
