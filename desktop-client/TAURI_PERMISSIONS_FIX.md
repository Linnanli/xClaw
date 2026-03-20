# Tauri 权限配置修复

## 问题

错误信息：
```
event.listen not allowed. Permissions associated with this command: core:event:allow-listen
```

## 原因

Tauri v2 采用了更严格的权限模型，所有 API 调用都需要在配置中明确授权。

## 解决方案

### 1. 更新 `tauri.conf.json`

在 `app.security` 中添加 `capabilities` 配置：

```json
{
  "app": {
    "security": {
      "capabilities": [
        {
          "identifier": "main-capability",
          "description": "Main application capabilities",
          "windows": ["main"],
          "permissions": [
            "core:event:allow-listen",
            "core:event:allow-emit",
            "core:window:allow-create",
            "core:window:allow-close",
            "core:window:allow-set-title",
            "core:window:allow-set-size",
            "core:window:allow-set-position",
            "core:window:allow-center",
            "core:window:allow-show",
            "core:window:allow-hide",
            "core:default"
          ]
        }
      ]
    }
  }
}
```

### 2. 权限说明

- `core:event:allow-listen` - 允许监听事件（解决当前错误）
- `core:event:allow-emit` - 允许发送事件
- `core:window:*` - 窗口管理权限
- `core:default` - 核心默认权限集

### 3. 自定义命令权限

所有 `#[tauri::command]` 标记的函数会自动获得调用权限，无需额外配置。

### 4. 验证

重启开发服务器：

```bash
cd desktop-client
cargo tauri dev
```

## 参考

- [Tauri v2 Security](https://v2.tauri.app/concept/security/)
- [Tauri Capabilities](https://v2.tauri.app/reference/config/#capabilities)
