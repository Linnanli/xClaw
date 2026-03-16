# 令牌更新说明

## 问题

每次启动后端时都会生成新的认证令牌，导致 Tauri 应用无法连接。

## 解决方案

### 方式 1: 使用浏览器访问（推荐）

1. 启动脚本会显示新的令牌 URL
2. 浏览器会自动打开，带有新的令牌
3. 令牌会自动保存到本地存储

### 方式 2: 手动更新 Tauri 应用

1. 查看后端日志获取新令牌
   ```bash
   grep "gateway" /tmp/backend.log | tail -1
   ```

2. 输出示例
   ```
   gateway   http://127.0.0.1:3000/?token=59c7c863fa5bd3eeffc94533cd70a3393251c3ada49a226146a5a61ba62d6743
   ```

3. 复制令牌值

4. 在浏览器控制台中运行
   ```javascript
   localStorage.setItem('gateway_auth_token', '59c7c863fa5bd3eeffc94533cd70a3393251c3ada49a226146a5a61ba62d6743');
   location.reload();
   ```

### 方式 3: 清除本地存储并重新访问

1. 在浏览器控制台中运行
   ```javascript
   localStorage.removeItem('gateway_auth_token');
   location.reload();
   ```

2. 使用新的令牌 URL 访问
   ```
   http://localhost:5173?token=59c7c863fa5bd3eeffc94533cd70a3393251c3ada49a226146a5a61ba62d6743
   ```

---

## 获取新令牌

### 从启动脚本输出

启动脚本会显示：
```
前端 URL（带令牌）:
  http://localhost:5173?token=59c7c863fa5bd3eeffc94533cd70a3393251c3ada49a226146a5a61ba62d6743
```

### 从后端日志

```bash
grep "gateway" /tmp/backend.log | tail -1
```

### 从浏览器

访问后端首页：
```
http://localhost:3000
```

页面会显示令牌输入框。

---

## 常见问题

### Q: 为什么每次启动都需要更新令牌？

**A**: 这是安全设计。每次启动后端时都会生成新的令牌，防止令牌泄露。

### Q: 如何避免每次都更新令牌？

**A**: 使用浏览器访问，令牌会自动保存到本地存储。

### Q: Tauri 应用为什么不能自动更新令牌？

**A**: Tauri 应用使用的是本地生成的令牌，与后端的令牌不同。这是架构设计的问题。

### Q: 如何修复这个问题？

**A**: 需要修改 Tauri 应用的认证系统，使其使用后端生成的令牌。这需要更多的开发工作。

---

## 临时解决方案

### 使用浏览器而不是 Tauri

```bash
# 启动脚本会自动打开浏览器
bash scripts/start-all.sh

# 浏览器会自动获取新的令牌
# 令牌会保存到本地存储
```

### 优点

- ✅ 自动获取新令牌
- ✅ 令牌自动保存
- ✅ 无需手动更新

### 缺点

- ❌ 不是原生应用
- ❌ 需要浏览器

---

## 长期解决方案

需要修改 Tauri 应用的认证系统：

1. 添加 Tauri 命令获取后端令牌
2. 在应用启动时自动获取令牌
3. 定期刷新令牌

这需要修改以下文件：
- `desktop-client/src/main.rs`
- `desktop-client/src/commands.rs`
- `desktop-client/src/api_client.rs`

---

## 相关文档

- `TOKEN_MANAGEMENT_GUIDE.md` - 令牌管理指南
- `TAURI_AND_FRONTEND_RELATIONSHIP.md` - Tauri 和前端的关系
- `COMPLETE_STARTUP_INSTRUCTIONS.md` - 完整启动说明

