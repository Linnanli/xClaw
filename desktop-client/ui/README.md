# Generate Page from Document

This is a code bundle for Generate Page from Document. The original project is available at https://www.figma.com/design/UlYUaddXXhCjpYCwybml6C/Generate-Page-from-Document.

## 🚀 快速开始

### 1. 安装依赖

```bash
npm i
```

### 2. 启动开发服务器

```bash
npm run dev
```

## 🧪 Cypress E2E 测试

### 安装 Cypress

**一键安装（推荐）：**

**macOS/Linux:**
```bash
chmod +x scripts/cypress/install.sh
./scripts/cypress/install.sh
```

**Windows:**
```cmd
scripts\cypress\install.bat
```

**或使用 npm 脚本：**
```bash
npm run install-cypress
```

### 运行测试

```bash
# 打开 Cypress UI（推荐用于开发）
npm run cypress:open

# 运行所有测试
npm run cypress:run

# 有界面地运行测试
npm run cypress:run:headed

# 使用特定浏览器运行
npm run cypress:run:chrome
npm run cypress:run:firefox

# 运行 E2E 测试
npm run e2e
```

### 镜像配置

- **默认镜像**：官方镜像（最可靠）
- **国内镜像**：如果下载速度慢，参考 [SWITCH_TO_CHINA_MIRROR.md](./SWITCH_TO_CHINA_MIRROR.md)
- **详细说明**：[scripts/cypress/README.md](./scripts/cypress/README.md)

## 📁 项目结构

```
desktop-client/ui/
├── scripts/
│   └── cypress/              # Cypress 安装脚本和文档
│       ├── install.sh        # macOS/Linux 安装脚本
│       ├── install.bat       # Windows 安装脚本
│       └── README.md         # Cypress 安装指南
├── src/                      # 源代码
├── .npmrc                    # NPM 镜像配置
├── .env.cypress              # Cypress 环境变量
├── cypress.config.ts         # Cypress 配置
├── package.json              # 项目依赖
└── README.md                 # 本文件
```

## 🔧 其他命令

```bash
# 单元测试
npm run test

# 单元测试 UI
npm run test:ui

# 代码覆盖率
npm run test:coverage

# 构建生产版本
npm run build
```

## 📚 相关文档

- [Cypress 安装指南](./scripts/cypress/README.md)
- [国内镜像切换指南](./SWITCH_TO_CHINA_MIRROR.md)
- [Cypress 官方文档](https://docs.cypress.io/)

## 💡 提示

- 首次安装 Cypress 可能需要 5-10 分钟
- 如果遇到问题，查看 [scripts/cypress/README.md](./scripts/cypress/README.md) 的故障排除部分
- 所有开发者应使用相同的 `.npmrc` 配置
