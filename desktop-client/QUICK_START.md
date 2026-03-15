# Desktop Client 快速开始指南

## 一次性设置（首次使用）

```bash
# 1. 进入项目目录
cd desktop-client

# 2. 安装前端依赖
cd src-ui
npm install
cd ..

# 完成！现在可以开始开发了
```

## 日常开发流程

### 启动开发模式（需要两个终端窗口）

**终端 1 - 启动 Vite 开发服务器：**
```bash
cd desktop-client/src-ui
npm run dev
```

输出应该显示：
```
  VITE v6.3.5  ready in XXX ms

  ➜  Local:   http://localhost:5173/
  ➜  press h to show help
```

**终端 2 - 启动 Tauri 应用：**
```bash
cd desktop-client
cargo tauri dev
```

**就这么简单！** 这会：
- ✅ 编译 Rust 后端
- ✅ 打开应用窗口
- ✅ 连接到 Vite 开发服务器
- ✅ 启用热重载

### 修改代码

**修改前端代码（React/TypeScript）：**
1. 编辑 `src-ui/src/` 中的任何文件
2. 保存文件
3. 浏览器自动刷新（1-2秒）
4. 立即看到效果

**修改后端代码（Rust）：**
1. 编辑 `src/` 中的任何文件
2. 保存文件
3. Cargo 自动重新编译（5-10秒）
4. 应用自动重启

### 停止开发

在两个终端分别按 `Ctrl+C` 停止服务。

## 常见操作

### 查看日志

- **前端日志**: 终端 1（Vite 输出）
- **后端日志**: 终端 2（Tauri/Rust 输出）
- **浏览器日志**: 按 F12 打开开发者工具

### 打开开发者工具

在应用窗口中按 `F12` 或 `Cmd+Option+I` (macOS)

### 清除缓存

```bash
# 清除 Rust 编译缓存
cargo clean

# 清除前端缓存
cd src-ui
rm -rf node_modules dist
npm install
cd ..
```

### 构建生产版本

```bash
# 先构建前端
cd desktop-client/src-ui
npm run build
cd ..

# 再构建应用
cargo tauri build
```

输出位置：
- macOS: `target/release/bundle/macos/`
- Linux: `target/release/bundle/deb/`
- Windows: `target/release/bundle/msi/`

## 开发技巧

### 1. 使用 TypeScript 类型提示

在 VS Code 中，TypeScript 会自动提供：
- 自动补全
- 类型检查
- 错误提示
- 重构支持

### 2. 使用 React DevTools

安装浏览器扩展：
- Chrome: [React Developer Tools](https://chrome.google.com/webstore/detail/react-developer-tools/fmkadmapgofadopljbjfkapdkoienihi)
- Firefox: [React Developer Tools](https://addons.mozilla.org/en-US/firefox/addon/react-devtools/)

### 3. 使用 Tailwind CSS IntelliSense

在 VS Code 中安装扩展：
- [Tailwind CSS IntelliSense](https://marketplace.visualstudio.com/items?itemName=bradlc.vscode-tailwindcss)

### 4. 快速定位组件

项目结构：
```
src-ui/src/app/
├── components/
│   ├── auth/          # 认证相关组件
│   ├── main/          # 主应用组件
│   ├── tabs/          # 各个Tab页面
│   └── ui/            # 通用UI组件
├── contexts/          # React Context
├── utils/
│   └── tauri.ts       # Tauri API封装
└── routes.tsx         # 路由配置
```

## 故障排除

### 问题：端口 5173 被占用

```bash
# macOS/Linux
lsof -ti:5173 | xargs kill -9

# 或修改端口（编辑 vite.config.ts）
```

### 问题：Tauri API 不可用

确保：
1. Vite 开发服务器正在运行（终端 1）
2. `cargo tauri dev` 正在运行（终端 2）
3. 浏览器连接到 http://localhost:5173

### 问题：热重载不工作

1. 检查两个终端是否都在运行
2. 查看终端 1 是否有错误
3. 尝试重启两个服务

### 问题：编译错误

```bash
# 清除缓存并重新编译
cargo clean
cd src-ui && npm run build && cd ..
cargo tauri dev
```

## 更多信息

- 详细构建指南: [DESKTOP_CLIENT_BUILD_GUIDE.md](../DESKTOP_CLIENT_BUILD_GUIDE.md)
- UI 迁移说明: [UI_MIGRATION_GUIDE.md](UI_MIGRATION_GUIDE.md)
- Tauri 文档: https://tauri.app/
- Vite 文档: https://vitejs.dev/
- React 文档: https://react.dev/

---

**记住：需要两个终端窗口，分别运行 `npm run dev` 和 `cargo tauri dev`！**
