# 启动脚本最终说明

**日期**: 2026-03-16  
**版本**: 最终版本

---

## 关键发现

### Tauri 和前端的关系

- ✅ `cargo tauri dev` **不会**自动启动前端
- ✅ `cargo tauri dev` **会**连接到已运行的前端开发服务器
- ✅ 前端必须在 Tauri 之前启动

### 启动脚本的优化

启动脚本现在正确地：
1. 启动后端服务
2. 启动前端开发服务器（只启动一次）
3. 启动 Tauri 客户端（连接到前端）

---

## 使用方法

### 一句话启动

```bash
export LLM_API_KEY="sk-d162b1775e514757b083a2dfe729d6e6" && bash scripts/start-all.sh
```

### 启动的服务

| 服务 | 端口 | 说明 |
|------|------|------|
| 后端 | 3000 | IronClaw API 服务器 |
| 前端 | 5173 | React 开发服务器 |
| Tauri | - | 桌面应用（连接到前端） |

---

## 访问应用

### 方式 1: Tauri 桌面应用（推荐）

Tauri 应用窗口会自动打开。

### 方式 2: 浏览器

浏览器会自动打开：
```
http://localhost:5173?token=...
```

### 方式 3: 手动访问

```
http://localhost:5173?token=36c1a0275ae2708954a402871a27255bcecf9aa24f7542a43ff0c5e1f651b19b
```

---

## 停止服务

按 `Ctrl+C` 停止所有服务。

---

## 相关文档

- `TAURI_AND_FRONTEND_RELATIONSHIP.md` - Tauri 和前端的关系
- `COMPLETE_STARTUP_INSTRUCTIONS.md` - 完整启动说明
- `TOKEN_MANAGEMENT_GUIDE.md` - 令牌管理指南

