# 管理后台前端菜单和功能清单

## 技术栈
- **React** + **TypeScript**
- **Ant Design** (antd) - UI 组件库
- **React Router** - 路由管理
- **Axios** - HTTP 客户端
- **Zustand** / **Redux Toolkit** - 状态管理

---

## 登录页面改造

### 当前状态
- 后端已有登录 API：`POST /api/auth/login`
- 支持用户名和密码登录
- 返回 JWT 令牌

### 前端登录页面需求

#### 页面布局
```
┌─────────────────────────────────────┐
│                                     │
│         IronClaw 管理后台           │
│                                     │
│   ┌─────────────────────────────┐   │
│   │  用户名                     │   │
│   │  [___________________]      │   │
│   │                             │   │
│   │  密码                       │   │
│   │  [___________________]      │   │
│   │                             │   │
│   │  [ ] 记住我                 │   │
│   │                             │   │
│   │  [      登录      ]         │   │
│   └─────────────────────────────┘   │
│                                     │
└─────────────────────────────────────┘
```

#### 功能要求
1. **用户名输入框**
   - 必填
   - 验证：非空
   - 提示：请输入用户名

2. **密码输入框**
   - 必填
   - 类型：password（隐藏输入）
   - 验证：非空
   - 提示：请输入密码

3. **记住我**（可选）
   - 勾选后保存用户名到 localStorage
   - 下次自动填充用户名

4. **登录按钮**
   - 点击后调用 `POST /api/auth/login`
   - 显示加载状态
   - 成功后保存 token 到 localStorage
   - 跳转到仪表盘

5. **错误处理**
   - 显示登录失败提示
   - 401：用户名或密码错误
   - 500：服务器错误

#### API 接口

**请求**：
```typescript
POST /api/auth/login
Content-Type: application/json

{
  "username": "admin",
  "password": "password123"
}
```

**响应**：
```typescript
// 成功
{
  "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "user": {
    "id": "uuid",
    "username": "admin",
    "email": "admin@example.com",
    "role": "admin"
  }
}

// 失败
{
  "error": "Invalid credentials"
}
```

---

## 菜单结构

### 一级菜单

```
管理后台
├── 📊 仪表盘 (Dashboard)
├── 👥 用户管理 (Users)
│   ├── 用户列表
│   ├── 角色管理
│   └── 权限管理
├── 🛡️ 安全策略 (Security)
│   ├── DLP 规则
│   ├── 敏感操作规则
│   └── 策略版本
├── 📝 审计日志 (Audit)
│   ├── 操作日志
│   └── 登录日志
├── 💻 客户端管理 (Clients)
│   ├── 客户端列表
│   └── 策略推送
├── 🧩 扩展管理 (Extensions)
│   ├── 技能管理
│   └── 扩展管理
├── ⚙️ 系统配置 (Settings)
│   ├── 基础配置
│   ├── 会话配置
│   └── 国密配置
└── 📈 统计报表 (Statistics)
    ├── 用户活动
    ├── DLP 统计
    └── 系统健康
```

---

## 详细功能清单

### 1. 📊 仪表盘 (Dashboard)

**路由**：`/dashboard`

**功能**：
- 显示关键指标卡片
  - 总用户数
  - 在线客户端数
  - 今日 DLP 拦截次数
  - 今日敏感操作次数
- 用户活动趋势图（最近 7 天）
- DLP 拦截趋势图（最近 7 天）
- 最近操作日志（最新 10 条）
- 系统健康状态

**API**：
- `GET /api/stats/dashboard` - 获取仪表盘数据

**组件**：
- `StatCard` - 统计卡片
- `TrendChart` - 趋势图表
- `RecentLogs` - 最近日志列表

---

### 2. 👥 用户管理 (Users)

#### 2.1 用户列表

**路由**：`/users`

**功能**：
- 用户列表表格
  - 列：ID、用户名、邮箱、角色、状态、创建时间、操作
  - 分页（每页 20 条）
  - 搜索（用户名、邮箱）
  - 过滤（角色、状态）
  - 排序（创建时间、用户名）
- 操作按钮
  - 新建用户
  - 编辑用户
  - 删除用户
  - 重置密码
  - 启用/禁用

**API**：
- `GET /api/users` - 获取用户列表
- `POST /api/users` - 创建用户
- `PUT /api/users/:id` - 更新用户
- `DELETE /api/users/:id` - 删除用户
- `POST /api/users/:id/reset-password` - 重置密码
- `POST /api/users/:id/toggle-status` - 启用/禁用

**表单字段**：
- 用户名（必填，唯一）
- 邮箱（必填，唯一）
- 密码（创建时必填）
- 角色（下拉选择）
- 状态（启用/禁用）

#### 2.2 角色管理

**路由**：`/users/roles`

**功能**：
- 角色列表表格
  - 列：ID、角色名、描述、权限数量、用户数量、操作
- 操作按钮
  - 新建角色
  - 编辑角色
  - 删除角色
  - 分配权限

**API**：
- `GET /api/roles` - 获取角色列表
- `POST /api/roles` - 创建角色
- `PUT /api/roles/:id` - 更新角色
- `DELETE /api/roles/:id` - 删除角色
- `POST /api/roles/:id/permissions` - 分配权限

**表单字段**：
- 角色名（必填，唯一）
- 描述
- 权限（多选）

#### 2.3 权限管理

**路由**：`/users/permissions`

**功能**：
- 权限列表表格
  - 列：ID、权限名、描述、资源、操作
- 权限分组显示
  - 用户管理权限
  - 角色管理权限
  - DLP 管理权限
  - 审计日志权限
  - 系统配置权限

**API**：
- `GET /api/permissions` - 获取权限列表

---

### 3. 🛡️ 安全策略 (Security)

#### 3.1 DLP 规则

**路由**：`/security/dlp-rules`

**功能**：
- DLP 规则列表表格
  - 列：ID、规则名、匹配模式、严重级别、动作、状态、操作
  - 分页
  - 搜索（规则名、模式）
  - 过滤（严重级别、动作、状态）
- 操作按钮
  - 新建规则
  - 编辑规则
  - 删除规则
  - 测试规则
  - 启用/禁用
  - 导入规则
  - 导出规则

**API**：
- `GET /api/dlp-rules` - 获取 DLP 规则列表
- `POST /api/dlp-rules` - 创建 DLP 规则
- `PUT /api/dlp-rules/:id` - 更新 DLP 规则
- `DELETE /api/dlp-rules/:id` - 删除 DLP 规则
- `POST /api/dlp-rules/test` - 测试 DLP 规则
- `POST /api/dlp-rules/import` - 导入 DLP 规则
- `GET /api/dlp-rules/export` - 导出 DLP 规则

**表单字段**：
- 规则名（必填）
- 匹配模式（正则表达式，必填）
- 替换规则（可选）
- 严重级别（下拉：low, medium, high, critical）
- 动作（下拉：redact, block, warn）
- 描述
- 状态（启用/禁用）

**测试规则对话框**：
- 输入测试文本
- 显示匹配结果
- 显示脱敏后的文本

#### 3.2 敏感操作规则

**路由**：`/security/sensitive-operations`

**功能**：
- 敏感操作规则列表表格
  - 列：ID、操作类型、需要批准、批准者角色、状态、操作
  - 分页
  - 搜索（操作类型）
  - 过滤（需要批准、状态）
- 操作按钮
  - 新建规则
  - 编辑规则
  - 删除规则
  - 启用/禁用

**API**：
- `GET /api/sensitive-operations` - 获取敏感操作规则列表
- `POST /api/sensitive-operations` - 创建敏感操作规则
- `PUT /api/sensitive-operations/:id` - 更新敏感操作规则
- `DELETE /api/sensitive-operations/:id` - 删除敏感操作规则

**表单字段**：
- 操作类型（下拉：file_delete, system_command, network_access）
- 需要批准（是/否）
- 批准者角色（多选）
- 描述
- 状态（启用/禁用）

#### 3.3 策略版本

**路由**：`/security/policy-versions`

**功能**：
- 策略版本列表表格
  - 列：版本号、发布时间、变更说明、签名状态、操作
  - 分页
- 操作按钮
  - 创建新版本
  - 查看版本详情
  - 版本对比
  - 回滚版本

**API**：
- `GET /api/policy-versions` - 获取策略版本列表
- `POST /api/policy-versions` - 创建新版本
- `GET /api/policy-versions/:id` - 获取版本详情
- `GET /api/policy-versions/:id/diff` - 版本对比
- `POST /api/policy-versions/:id/rollback` - 回滚版本

**表单字段**：
- 变更说明（必填）
- 是否签名（如果启用国密）

---

### 4. 📝 审计日志 (Audit)

#### 4.1 操作日志

**路由**：`/audit/logs`

**功能**：
- 审计日志列表表格
  - 列：时间、用户、操作类型、资源、IP 地址、状态、详情
  - 分页（每页 50 条）
  - 时间范围过滤（今天、最近 7 天、最近 30 天、自定义）
  - 用户过滤
  - 操作类型过滤
  - 状态过滤（成功、失败）
  - 搜索（资源、IP）
- 操作按钮
  - 查看详情
  - 导出日志（CSV/JSON）

**API**：
- `GET /api/audit-logs` - 获取审计日志列表
- `GET /api/audit-logs/:id` - 获取日志详情
- `GET /api/audit-logs/export` - 导出日志
- `GET /api/audit-logs/stats` - 获取日志统计

**日志详情对话框**：
- 时间
- 用户
- 操作类型
- 资源
- IP 地址
- User-Agent
- 请求参数
- 响应结果
- 错误信息（如果失败）

#### 4.2 登录日志

**路由**：`/audit/login-logs`

**功能**：
- 登录日志列表表格
  - 列：时间、用户、IP 地址、设备、状态、操作
  - 分页
  - 时间范围过滤
  - 用户过滤
  - 状态过滤（成功、失败）
- 操作按钮
  - 查看详情
  - 导出日志

**API**：
- `GET /api/audit-logs?type=login` - 获取登录日志

---

### 5. 💻 客户端管理 (Clients)

#### 5.1 客户端列表

**路由**：`/clients`

**功能**：
- 客户端列表表格
  - 列：客户端 ID、用户、版本、操作系统、IP 地址、最后活动时间、在线状态、操作
  - 分页
  - 搜索（客户端 ID、用户）
  - 过滤（在线状态、操作系统）
  - 排序（最后活动时间）
- 操作按钮
  - 查看详情
  - 推送策略
  - 强制下线

**API**：
- `GET /api/clients` - 获取客户端列表
- `GET /api/clients/:id` - 获取客户端详情
- `POST /api/clients/:id/push-policy` - 推送策略
- `POST /api/clients/:id/force-offline` - 强制下线

**客户端详情对话框**：
- 客户端 ID
- 用户信息
- 客户端版本
- 操作系统
- IP 地址
- 最后活动时间
- 策略版本
- 在线状态

#### 5.2 策略推送

**路由**：`/clients/push-policy`

**功能**：
- 选择客户端（多选）
- 选择策略版本
- 推送策略
- 显示推送结果

**API**：
- `POST /api/clients/batch-push-policy` - 批量推送策略

---

### 6. 🧩 扩展管理 (Extensions)

#### 6.1 技能管理

**路由**：`/extensions/skills`

**功能**：
- 技能列表表格
  - 列：ID、技能名、版本、作者、审核状态、签名状态、操作
  - 分页
  - 搜索（技能名、作者）
  - 过滤（审核状态、签名状态）
- 操作按钮
  - 查看详情
  - 审核通过
  - 审核拒绝
  - 签名技能
  - 发布技能

**API**：
- `GET /api/skills` - 获取技能列表
- `GET /api/skills/:id` - 获取技能详情
- `POST /api/skills/:id/approve` - 审核通过
- `POST /api/skills/:id/reject` - 审核拒绝
- `POST /api/skills/:id/sign` - 签名技能
- `POST /api/skills/:id/publish` - 发布技能

**技能详情对话框**：
- 技能名
- 版本
- 作者
- 描述
- 审核状态
- 签名状态
- 创建时间
- 更新时间

#### 6.2 扩展管理

**路由**：`/extensions/extensions`

**功能**：
- 扩展列表表格
  - 列：ID、扩展名、版本、作者、审核状态、签名状态、操作
  - 分页
  - 搜索（扩展名、作者）
  - 过滤（审核状态、签名状态）
- 操作按钮
  - 查看详情
  - 审核通过
  - 审核拒绝
  - 签名扩展
  - 发布扩展

**API**：
- `GET /api/extensions` - 获取扩展列表
- `GET /api/extensions/:id` - 获取扩展详情
- `POST /api/extensions/:id/approve` - 审核通过
- `POST /api/extensions/:id/reject` - 审核拒绝
- `POST /api/extensions/:id/sign` - 签名扩展
- `POST /api/extensions/:id/publish` - 发布扩展

---

### 7. ⚙️ 系统配置 (Settings)

#### 7.1 基础配置

**路由**：`/settings/basic`

**功能**：
- 配置表单
  - 系统名称
  - 系统描述
  - 管理员邮箱
  - 日志级别（下拉：debug, info, warn, error）
  - 日志保留天数
- 保存按钮

**API**：
- `GET /api/system-config` - 获取系统配置
- `PUT /api/system-config` - 更新系统配置

#### 7.2 会话配置

**路由**：`/settings/session`

**功能**：
- 配置表单
  - 会话超时时间（分钟）
  - 令牌刷新间隔（分钟）
  - 最大并发会话数
  - 是否允许多设备登录
- 保存按钮

**API**：
- `GET /api/system-config` - 获取系统配置
- `PUT /api/system-config` - 更新系统配置

#### 7.3 国密配置

**路由**：`/settings/gm`

**功能**：
- 配置表单
  - 是否启用国密算法
  - SM2 公钥
  - SM2 私钥（加密存储）
  - SM3 哈希算法
  - SM4 加密算法
- 保存按钮
- 测试按钮（测试国密算法是否正常）

**API**：
- `GET /api/system-config` - 获取系统配置
- `PUT /api/system-config` - 更新系统配置
- `POST /api/system-config/test-gm` - 测试国密算法

---

### 8. 📈 统计报表 (Statistics)

#### 8.1 用户活动

**路由**：`/statistics/user-activity`

**功能**：
- 时间范围选择（今天、最近 7 天、最近 30 天、自定义）
- 统计卡片
  - 活跃用户数
  - 登录次数
  - 操作次数
- 用户活动趋势图（折线图）
- 用户操作排行榜（表格）

**API**：
- `GET /api/stats/user-activity` - 获取用户活动统计

#### 8.2 DLP 统计

**路由**：`/statistics/dlp`

**功能**：
- 时间范围选择
- 统计卡片
  - 脱敏次数
  - 阻止次数
  - 警告次数
- DLP 拦截趋势图（折线图）
- 规则触发排行榜（表格）
- 敏感数据类型分布（饼图）

**API**：
- `GET /api/stats/dlp` - 获取 DLP 统计

#### 8.3 系统健康

**路由**：`/statistics/system-health`

**功能**：
- 实时监控
  - 在线客户端数
  - 数据库连接数
  - 内存使用情况
  - CPU 使用情况
- 系统健康趋势图（折线图）
- 错误日志统计（表格）

**API**：
- `GET /api/stats/system-health` - 获取系统健康统计

---

## 通用功能

### 顶部导航栏
- Logo + 系统名称
- 用户信息下拉菜单
  - 个人资料
  - 修改密码
  - 退出登录

### 侧边栏菜单
- 可折叠
- 图标 + 文字
- 当前选中高亮
- 支持二级菜单

### 面包屑导航
- 显示当前页面路径
- 可点击返回上级

### 权限控制
- 根据用户角色显示/隐藏菜单
- 根据权限显示/隐藏操作按钮
- 无权限时显示 403 页面

### 错误处理
- 401：跳转到登录页
- 403：显示无权限页面
- 404：显示页面不存在
- 500：显示服务器错误

### 加载状态
- 页面加载：显示骨架屏
- 表格加载：显示加载动画
- 按钮加载：显示加载图标

### 通知提示
- 成功提示（绿色）
- 错误提示（红色）
- 警告提示（黄色）
- 信息提示（蓝色）

---

## 技术实现要点

### 1. 路由配置
```typescript
// src/router/index.tsx
import { createBrowserRouter } from 'react-router-dom';

const router = createBrowserRouter([
  {
    path: '/login',
    element: <LoginPage />,
  },
  {
    path: '/',
    element: <Layout />,
    children: [
      { path: 'dashboard', element: <Dashboard /> },
      { path: 'users', element: <UserList /> },
      { path: 'users/roles', element: <RoleList /> },
      // ... 更多路由
    ],
  },
]);
```

### 2. API 客户端
```typescript
// src/api/client.ts
import axios from 'axios';

const apiClient = axios.create({
  baseURL: 'http://localhost:3000/api',
  timeout: 10000,
});

// 请求拦截器：添加 token
apiClient.interceptors.request.use((config) => {
  const token = localStorage.getItem('token');
  if (token) {
    config.headers.Authorization = `Bearer ${token}`;
  }
  return config;
});

// 响应拦截器：处理错误
apiClient.interceptors.response.use(
  (response) => response,
  (error) => {
    if (error.response?.status === 401) {
      // 跳转到登录页
      window.location.href = '/login';
    }
    return Promise.reject(error);
  }
);
```

### 3. 状态管理
```typescript
// src/store/auth.ts
import { create } from 'zustand';

interface AuthState {
  user: User | null;
  token: string | null;
  login: (username: string, password: string) => Promise<void>;
  logout: () => void;
}

export const useAuthStore = create<AuthState>((set) => ({
  user: null,
  token: localStorage.getItem('token'),
  login: async (username, password) => {
    const response = await apiClient.post('/auth/login', {
      username,
      password,
    });
    const { token, user } = response.data;
    localStorage.setItem('token', token);
    set({ token, user });
  },
  logout: () => {
    localStorage.removeItem('token');
    set({ token: null, user: null });
  },
}));
```

### 4. 权限控制
```typescript
// src/components/PermissionGuard.tsx
import { useAuthStore } from '@/store/auth';

interface PermissionGuardProps {
  permission: string;
  children: React.ReactNode;
}

export const PermissionGuard: React.FC<PermissionGuardProps> = ({
  permission,
  children,
}) => {
  const { user } = useAuthStore();
  
  if (!user?.permissions.includes(permission)) {
    return null;
  }
  
  return <>{children}</>;
};
```

---

## 开发优先级

### P0 - 核心功能（第一阶段，2周）
1. 登录页面
2. 布局框架（顶部导航 + 侧边栏 + 内容区）
3. 仪表盘
4. 用户管理（列表、创建、编辑、删除）
5. DLP 规则管理（列表、创建、编辑、删除）

### P1 - 重要功能（第二阶段，2周）
6. 角色和权限管理
7. 敏感操作规则管理
8. 审计日志查看
9. 策略版本管理
10. 客户端管理

### P2 - 增强功能（第三阶段，2周）
11. 技能和扩展管理
12. 系统配置
13. 统计报表
14. 通知和告警

---

## 总结

**菜单结构**：8 个一级菜单，20+ 个二级页面

**核心功能**：
1. 用户和权限管理（RBAC）
2. DLP 和敏感操作规则管理
3. 审计日志查看
4. 客户端管理和策略推送
5. 统计报表

**技术栈**：
- React + TypeScript
- Ant Design
- React Router
- Axios
- Zustand

**开发时间**：约 6 周（全职开发）

**下一步**：
1. 确认菜单结构和功能清单
2. 设计 UI 原型（可选）
3. 搭建前端项目
4. 按优先级开发功能模块
