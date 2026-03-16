# Tauri 和前端的关系

## 问题

`cargo tauri dev` 启动客户端时，是否会一并启动 web 前端？

## 答案

**是的，但有细节差别。**

---

## 工作原理

### Tauri 配置

```json
{
  "build": {
    "devUrl": "http://localhost:5173"
  }
}
```

### 启动流程

1. **`cargo tauri dev` 启动时**
   - Tauri 编译 Rust 后端代码
   - Tauri 打开应用窗口
   - Tauri 连接到 `http://localhost:5173`（前端开发服务器）

2. **前端开发服务器**
   - 需要单独启动
   - Tauri 会连接到已运行的前端服务器
   - 不会自动启动前端

---

## 三种启动方式

### 方式 1: 只启动 Tauri（需要前端已运行）

```bash
cd desktop-client
cargo tauri dev
```

**前提**: 前端开发服务器必须已经运行在 `http://localhost:5173`

### 方式 2: 先启动前端，再启动 Tauri

```bash
# 终端 1: 启动前端
cd desktop-client/src-ui
npm run dev

# 终端 2: 启动 Tauri
cd desktop-client
cargo tauri dev
```

### 方式 3: 使用启动脚本（推荐）

```bash
export LLM_API_KEY="sk-..."
bash scripts/start-all.sh
```

启动脚本会自动：
1. 启动后端服务
2. 启动前端开发服务器
3. 启动 Tauri 客户端

---

## 启动脚本的优化

### 之前的做法（重复启动）

```bash
# 启动前端
npm run dev &

# 启动 Tauri（会连接到前端）
cargo tauri dev &
```

**问题**: 前端被启动了两次

### 现在的做法（正确）

```bash
# 启动前端
npm run dev &
FRONTEND_PID=$!

# 启动 Tauri（连接到已运行的前端）
cargo tauri dev &
TAURI_PID=$!
```

**优点**: 
- ✅ 前端只启动一次
- ✅ Tauri 连接到同一个前端实例
- ✅ 资源使用更少
- ✅ 启动更快

---

## 架构图

```
┌─────────────────────────────────────────┐
│         启动脚本 (start-all.sh)          │
└─────────────────────────────────────────┘
                    │
        ┌───────────┼───────────┐
        │           │           │
        ▼           ▼           ▼
    ┌────────┐ ┌────────┐ ┌──────────┐
    │ 后端   │ │ 前端   │ │ Tauri    │
    │ :3000  │ │ :5173  │ │ 客户端   │
    └────────┘ └────────┘ └──────────┘
                    ▲           │
                    │           │
                    └───────────┘
                  (Tauri 连接到前端)
```

---

## 关键点

### 1. Tauri 不启动前端

`cargo tauri dev` **不会**自动启动前端开发服务器。

### 2. Tauri 连接到前端

Tauri 会连接到已经运行的前端开发服务器（`http://localhost:5173`）。

### 3. 前端必须先启动

启动 Tauri 前，前端开发服务器必须已经运行。

### 4. 共享同一个前端

Tauri 和浏览器可以共享同一个前端开发服务器实例。

---

## 启动顺序

### 正确的顺序

```
1. 启动后端服务
   ↓
2. 启动前端开发服务器
   ↓
3. 启动 Tauri 客户端
```

### 错误的顺序

```
❌ 直接启动 Tauri（前端未运行）
   → Tauri 无法连接到前端
   → 应用窗口显示空白
```

---

## 访问应用

### 通过 Tauri 客户端

- Tauri 应用窗口会显示前端界面
- 自动连接到后端服务

### 通过浏览器

- 访问 `http://localhost:5173?token=...`
- 使用同一个前端开发服务器

### 两者共享前端

- 都连接到 `http://localhost:5173`
- 实时同步更新（热重载）

---

## 常见问题

### Q: Tauri 启动后显示空白

**A**: 前端开发服务器未运行

解决方案：
```bash
# 在另一个终端启动前端
cd desktop-client/src-ui
npm run dev
```

### Q: 如何同时开发 Tauri 和 Web

**A**: 使用启动脚本或在两个终端中分别启动

```bash
# 终端 1: 启动前端
cd desktop-client/src-ui
npm run dev

# 终端 2: 启动 Tauri
cd desktop-client
cargo tauri dev

# 终端 3: 启动后端
export LLM_API_KEY="..."
cargo run -- run --no-onboard < /dev/null
```

### Q: 前端修改后 Tauri 会自动更新吗

**A**: 是的，因为 Tauri 连接到前端开发服务器，会自动热重载

---

## 相关文件

- `desktop-client/tauri.conf.json` - Tauri 配置
- `scripts/start-all.sh` - 启动脚本
- `COMPLETE_STARTUP_INSTRUCTIONS.md` - 完整启动说明

---

## 总结

| 方面 | 说明 |
|------|------|
| **Tauri 启动前端** | ❌ 不会自动启动 |
| **Tauri 连接前端** | ✅ 会连接到已运行的前端 |
| **前端启动顺序** | 必须在 Tauri 之前启动 |
| **共享前端** | ✅ Tauri 和浏览器可以共享 |
| **热重载** | ✅ 前端修改会自动更新 |

