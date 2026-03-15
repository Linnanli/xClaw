# Desktop Client UI Migration Guide

## 概述

本项目已将UI从原生HTML/JavaScript迁移到React + TypeScript + Vite架构，同时保留Tauri桌面集成。

## 目录结构

```
desktop-client/
├── src/                    # Rust后端代码 (Tauri)
├── src-ui-backup/          # 原有UI备份
├── src-ui-new/             # 新的React UI
│   ├── src/
│   │   ├── app/
│   │   │   ├── components/
│   │   │   │   ├── auth/          # 认证组件
│   │   │   │   ├── main/          # 主应用组件
│   │   │   │   ├── tabs/          # 各个Tab组件
│   │   │   │   └── ui/            # shadcn/ui组件库
│   │   │   ├── contexts/          # React Context
│   │   │   ├── utils/
│   │   │   │   └── tauri.ts       # Tauri API集成层
│   │   │   ├── App.tsx
│   │   │   └── routes.tsx
│   │   ├── styles/                # 样式文件
│   │   └── main.tsx               # 入口文件
│   ├── package.json
│   └── vite.config.ts
└── tauri.conf.json         # Tauri配置
```

## 技术栈

- **前端框架**: React 18 + TypeScript
- **构建工具**: Vite 6
- **UI组件**: Radix UI + Tailwind CSS
- **路由**: React Router 7
- **桌面集成**: Tauri 2.0
- **主题**: 支持深色/浅色模式切换

## 安装依赖

```bash
cd desktop-client/src-ui-new
npm install
```

## 开发模式

```bash
# 在项目根目录运行
cd desktop-client
cargo tauri dev
```

这将：
1. 启动Vite开发服务器 (http://localhost:5173)
2. 启动Tauri应用并加载前端
3. 支持热重载

## 构建生产版本

```bash
cd desktop-client
cargo tauri build
```

## Tauri API集成

所有Tauri后端调用都通过 `src/app/utils/tauri.ts` 进行：

```typescript
import { authApi, threadApi, extensionApi, routineApi } from '@/app/utils/tauri';

// 认证
await authApi.unlockApp(password);

// 对话
await threadApi.createThread();
await threadApi.sendMessage(threadId, content);

// 扩展
await extensionApi.installExtension(metadata);

// 日程
await routineApi.createRoutine(name, description, trigger, actions);
```

## 已集成的功能

- ✅ 认证系统 (PasswordLogin, PasswordSetup)
- ✅ 聊天界面 (ChatTab)
- ✅ 扩展管理 (ExtensionsTab)
- ✅ 日程管理 (RoutinesTab)
- ✅ 主题切换 (深色/浅色模式)
- ✅ 响应式布局

## 待完成的功能

- ⏳ MemoryTab (记忆管理)
- ⏳ JobsTab (任务管理)
- ⏳ SkillsTab (技能管理)
- ⏳ LogsTab (日志查看)

## 注意事项

1. **Tauri API可用性**: 确保在Tauri环境中运行，否则API调用会失败
2. **会话管理**: 使用sessionStorage存储session_id
3. **错误处理**: 所有API调用都包含try-catch错误处理
4. **类型安全**: 使用TypeScript确保类型安全

## 调试

在浏览器开发者工具中查看：
- Console: 查看日志和错误
- Network: 查看API调用（虽然是IPC而非HTTP）
- React DevTools: 查看组件状态

## 迁移对比

| 特性 | 旧版 (src-ui) | 新版 (src-ui-new) |
|------|--------------|------------------|
| 框架 | 原生HTML/JS | React + TypeScript |
| 构建 | 无 | Vite |
| 样式 | 原生CSS | Tailwind CSS |
| 组件 | 无 | Radix UI (50+组件) |
| 路由 | 无 | React Router |
| 状态管理 | 全局变量 | React Hooks + Context |
| 类型检查 | 无 | TypeScript |
| 开发体验 | 基础 | 热重载 + 类型提示 |

## 相关文件

- `tauri.conf.json`: Tauri配置，指向新的UI目录
- `src-ui-backup/`: 原有UI的完整备份
- `DESKTOP_CLIENT_UI_FEATURES.md`: UI功能清单
