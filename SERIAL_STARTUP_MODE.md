# 串行启动模式

## 功能说明

`start-all.sh` 现在支持两种启动模式：

### 1. 并行模式（默认）

```bash
./scripts/start-all.sh
```

- 所有服务并行启动
- 启动速度快
- 编译进度不可见（后台编译）
- 适合后续运行（增量编译快）

### 2. 串行模式（新增）

```bash
./scripts/start-all.sh --serial
# 或
./scripts/start-all.sh -s
```

- 逐个启动服务
- 实时显示编译进度
- 清楚知道当前在编译哪个服务
- 推荐首次运行使用

## 使用场景

### 首次运行（推荐串行模式）

首次运行需要完整编译所有依赖（986 个 crates），耗时 10-20 分钟。使用串行模式可以：

- 看到实时编译进度
- 了解当前编译状态
- 避免"光等着"的焦虑

```bash
./scripts/start-all.sh --serial
```

### 后续运行（推荐并行模式）

后续运行只需增量编译（5-10 秒），使用并行模式可以：

- 更快启动所有服务
- 节省等待时间

```bash
./scripts/start-all.sh
```

## 性能影响

**串行模式不会影响编译性能**：

- 串行只是"等待"，不是"限制"
- 编译本身的速度不变
- 只是改变了启动顺序

**对比**：

| 模式 | 首次运行（完整编译） | 后续运行（增量编译） |
|------|---------------------|---------------------|
| 并行 | 15-20 分钟 | 30-60 秒 |
| 串行 | 15-20 分钟 | 60-90 秒 |

**结论**：串行模式只增加少量等待时间，但提供了更好的用户体验。

## 实现细节

### 串行启动函数

在 `admin-backend/scripts/common.sh` 中新增：

- `start_tauri_serial()` - 启动 Tauri 并实时显示编译进度
- `start_admin_backend_serial()` - 启动 Admin Backend 并实时显示编译进度

### 启动流程

#### 并行模式

```
启动 Tauri（后台）
  ↓
等待 Tauri 编译完成（智能检测）
  ↓
启动 Admin Backend（后台）
  ↓
启动 Admin Frontend（后台）
  ↓
完成
```

#### 串行模式

```
启动 Tauri
  ↓
实时显示编译进度
  ↓
等待 Tauri 完全启动
  ↓
启动 Admin Backend
  ↓
实时显示编译进度
  ↓
等待 Admin Backend 完全启动
  ↓
启动 Admin Frontend
  ↓
完成
```

## 相关文档

- [STARTUP_QUICK_REFERENCE.md](STARTUP_QUICK_REFERENCE.md) - 启动快速参考
- [RUST_COMPILATION_EXPLAINED.md](RUST_COMPILATION_EXPLAINED.md) - Rust 编译机制说明
- [QUICK_START.md](QUICK_START.md) - 完整启动指南

