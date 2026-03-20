# 路径别名迁移总结

## 问题

使用相对路径导入模块时,路径过长且难以维护:

```typescript
// ❌ 问题代码
import { API_BASE_URL } from '../../config/api';
import { useTheme } from '../../contexts/ThemeContext';
import { createSseClient } from '../utils/sse';
```

## 解决方案

配置路径别名,使用简洁的绝对路径:

```typescript
// ✅ 改进后
import { API_BASE_URL } from '@config/api';
import { useTheme } from '@contexts/ThemeContext';
import { createSseClient } from '@utils/sse';
```

## 修改的文件

### 新增配置文件

1. **`tsconfig.json`** - TypeScript 路径映射配置
   - 提供 IDE 自动补全和跳转支持
   - 配置所有路径别名

2. **`tsconfig.node.json`** - Node 环境配置
   - 用于 Vite 和 Vitest 配置文件

3. **`PATH_ALIASES.md`** - 路径别名使用文档
   - 详细的使用指南
   - 示例和最佳实践

### 修改的配置文件

1. **`vite.config.ts`**
   - 添加路径别名配置
   - 支持以下别名:
     - `@/*` → `./src/*`
     - `@app/*` → `./src/app/*`
     - `@config/*` → `./src/app/config/*`
     - `@components/*` → `./src/app/components/*`
     - `@hooks/*` → `./src/app/hooks/*`
     - `@utils/*` → `./src/app/utils/*`
     - `@contexts/*` → `./src/app/contexts/*`
     - `@services/*` → `./src/app/services/*`

### 更新的源文件

所有使用相对路径的文件已更新为使用别名:

1. **`src/app/components/DlpStatusIndicator.tsx`**
   - `../hooks/useDlpScan` → `@hooks/useDlpScan`

2. **`src/app/hooks/useDlpScan.ts`**
   - `../config/api` → `@config/api`

3. **`src/app/hooks/useAiChat.ts`**
   - `../utils/sse` → `@utils/sse`
   - `../utils/tokenManager` → `@utils/tokenManager`
   - `./useDlpScan` → `@hooks/useDlpScan`
   - `../config/api` → `@config/api`

4. **`src/app/components/tabs/SettingsTab.tsx`**
   - `../../contexts/ThemeContext` → `@contexts/ThemeContext`
   - `../../utils/tauri` → `@utils/tauri`
   - `../../config/api` → `@config/api`

5. **`src/app/components/tabs/LogsTab.tsx`**
   - `../../contexts/ThemeContext` → `@contexts/ThemeContext`
   - `../../utils/tauri` → `@utils/tauri`
   - `../../utils/sse` → `@utils/sse`
   - `../../config/api` → `@config/api`

6. **`src/app/components/tabs/ChatTabWithAiSdk.tsx`**
   - `../../config/api` → `@config/api`

## 路径别名规则

| 导入类型 | 别名 | 示例 |
|---------|------|------|
| 配置文件 | `@config/*` | `@config/api` |
| React 组件 | `@components/*` | `@components/ui/Button` |
| React Hooks | `@hooks/*` | `@hooks/useAiChat` |
| 工具函数 | `@utils/*` | `@utils/sse` |
| React Contexts | `@contexts/*` | `@contexts/ThemeContext` |
| 服务层 | `@services/*` | `@services/apiClient` |

## 优点

1. **更简洁**: 不需要计算 `../../` 的层级
2. **更清晰**: 一眼就能看出导入的模块类型
3. **更易维护**: 移动文件时不需要更新导入路径
4. **IDE 支持**: 自动补全和跳转到定义
5. **一致性**: 整个项目使用统一的导入风格

## 验证

启动开发服务器验证配置:

```bash
cd desktop-client/src-ui
npm run dev
```

如果没有报错,说明配置成功。

## IDE 支持

### VS Code

VS Code 会自动识别 `tsconfig.json`,提供:
- ✅ 自动补全路径
- ✅ 跳转到定义 (Cmd/Ctrl + Click)
- ✅ 重命名重构

### WebStorm

WebStorm 也会自动识别路径映射,无需额外配置。

## 最佳实践

1. **统一使用别名**: 避免混用相对路径和别名
2. **同级导入**: 同一目录下的文件可以使用 `./`
3. **跨级导入**: 跨多级目录时使用别名

```typescript
// ✅ 推荐
import { Button } from './Button';           // 同级
import { API_BASE_URL } from '@config/api';  // 跨级

// ❌ 不推荐
import { API_BASE_URL } from '../../config/api';  // 跨级使用相对路径
```

## 相关文档

- `desktop-client/src-ui/PATH_ALIASES.md` - 详细使用指南
- `desktop-client/API_PORT_FIX.md` - API 端口修复文档
- `desktop-client/ARCHITECTURE_EVOLUTION.md` - 架构演进方案

## 总结

通过配置路径别名,我们:
1. ✅ 解决了相对路径过长的问题
2. ✅ 提高了代码可读性和可维护性
3. ✅ 统一了项目的导入风格
4. ✅ 改善了开发体验 (IDE 支持)

现在可以愉快地使用 `@config/api` 而不是 `../../config/api` 了! 🎉
