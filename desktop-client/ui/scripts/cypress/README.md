# Cypress 安装和配置指南

## 📋 快速开始

### 一键安装

**macOS/Linux:**
```bash
cd desktop-client/ui
chmod +x scripts/cypress/install.sh
./scripts/cypress/install.sh
```

**Windows:**
```cmd
cd desktop-client\ui
scripts\cypress\install.bat
```

### 或使用 npm 脚本

```bash
cd desktop-client/ui
npm run install-cypress
```

## 📁 文件说明

| 文件 | 说明 |
|------|------|
| `install.sh` | macOS/Linux 安装脚本 |
| `install.bat` | Windows 安装脚本 |
| `README.md` | 本文件 |

## 🔧 配置文件

项目根目录中的配置文件：

| 文件 | 位置 | 说明 |
|------|------|------|
| `.npmrc` | `desktop-client/ui/.npmrc` | NPM 镜像配置 |
| `.env.cypress` | `desktop-client/ui/.env.cypress` | Cypress 环境变量 |

## 🌍 镜像源配置

### 当前配置

默认使用**官方镜像**（最可靠）：
```
registry: https://registry.npmjs.org/
cypress_download_mirror: https://download.cypress.io
```

### 切换到国内镜像

如果官方镜像下载速度慢，可以编辑 `.npmrc` 文件切换到国内镜像。

**推荐：阿里云镜像（npmmirror）**
```ini
registry=https://registry.npmmirror.com
cypress_download_mirror=https://cdn.npmmirror.com/binaries/cypress/
cypress_download_path_template=${endpoint}/desktop/${platform}-${arch}/cypress.zip
```

详见：[SWITCH_TO_CHINA_MIRROR.md](../../SWITCH_TO_CHINA_MIRROR.md)

## 🚀 常用命令

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

## ✅ 验证安装

安装完成后，验证 Cypress 是否正确安装：

```bash
npx cypress --version
```

应该输出类似：
```
Cypress: 15.12.0 (或更新版本)
```

## 🐛 故障排除

### 问题1：下载超时

**解决方案：**
```bash
npm install cypress --save-dev --timeout=600000
```

### 问题2：404 错误

**原因：** 镜像源中没有该版本

**解决方案：**
1. 检查 `.npmrc` 中的 `cypress_download_path_template` 是否正确
2. 尝试其他镜像源
3. 切换回官方镜像

### 问题3：权限错误

**解决方案：**
```bash
# 修复 npm 权限
mkdir ~/.npm-global
npm config set prefix '~/.npm-global'
export PATH=~/.npm-global/bin:$PATH
```

### 问题4：Cypress 二进制文件损坏

**解决方案：**
```bash
# 清除 Cypress 缓存
rm -rf ~/.cache/Cypress
# 重新安装
npm install cypress --save-dev
```

## 📚 更多资源

- [Cypress 官方文档](https://docs.cypress.io/)
- [Cypress GitHub](https://github.com/cypress-io/cypress)
- [npmmirror 镜像说明](https://npmmirror.com/)

## 💡 提示

- 首次安装可能需要 5-10 分钟，请耐心等待
- 如果网络不稳定，可以多次尝试
- 建议在公司网络或家庭网络下安装，避免使用移动热点
- 安装完成后，Cypress 会缓存二进制文件，后续使用会更快

## 🆘 需要帮助？

如果遇到问题，请：
1. 查看本文档的故障排除部分
2. 查看 [SWITCH_TO_CHINA_MIRROR.md](../../SWITCH_TO_CHINA_MIRROR.md)
3. 查看 Cypress 官方文档
