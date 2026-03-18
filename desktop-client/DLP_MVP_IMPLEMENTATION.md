# DLP MVP UI 实现指南

## 目标

用 3 小时实现最小可用的 DLP UI，让功能从"技术可用"变为"产品可用"。

## 需要实现的组件

### 1. DLP 警告 Toast（1.5小时）

**文件**：`desktop-client/src-ui/src/app/components/DlpWarningToast.tsx` ✅ 已创建

**集成位置**：聊天界面（ChatTab.tsx）

**集成代码**：
```typescript
import { DlpWarningToast } from '../components/DlpWarningToast';

function ChatTab() {
  const { dlpWarning, setDlpWarning } = useAiChat({ ... });
  
  return (
    <div>
      {/* 聊天界面 */}
      
      {/* DLP 警告 Toast */}
      {dlpWarning && (
        <DlpWarningToast
          redactedCount={dlpWarning.redacted}
          blockedCount={dlpWarning.blocked}
          onClose={() => setDlpWarning(null)}
        />
      )}
    </div>
  );
}
```

**效果**：
- 检测到敏感信息时，右上角显示黄色警告框
- 显示脱敏和阻止的数量
- 3秒后自动消失
- 用户可以手动关闭

### 2. DLP 状态指示器（1.5小时）

**文件**：`desktop-client/src-ui/src/app/components/DlpStatusIndicator.tsx` ✅ 已创建

**集成位置**：消息输入框旁边

**集成代码**：
```typescript
import { DlpStatusIndicator } from '../components/DlpStatusIndicator';

function ChatInput() {
  return (
    <div className="flex items-center gap-2">
      <input className="flex-1" />
      <DlpStatusIndicator />
      <button>发送</button>
    </div>
  );
}
```

**效果**：
- 显示 🛡️ 图标和"DLP 已启用/已禁用"文字
- 绿色（启用）/ 灰色（禁用）
- 鼠标悬停显示提示信息

## 实施步骤

### 步骤 1：找到聊天界面组件（15分钟）

```bash
# 搜索聊天界面组件
find desktop-client/src-ui/src -name "*Chat*.tsx" -o -name "*chat*.tsx"
```

可能的文件：
- `ChatTab.tsx`
- `ChatInterface.tsx`
- `MessageInput.tsx`

### 步骤 2：集成 DLP 警告 Toast（45分钟）

1. 导入组件
2. 从 useAiChat 获取 dlpWarning 状态
3. 条件渲染 Toast
4. 测试显示效果

### 步骤 3：集成 DLP 状态指示器（45分钟）

1. 找到消息输入框组件
2. 导入 DlpStatusIndicator
3. 添加到输入框旁边
4. 调整样式和布局

### 步骤 4：测试和调整（45分钟）

1. 测试脱敏场景
2. 测试阻止场景
3. 测试 UI 显示
4. 调整样式和动画

## 验收标准

### 功能验收
- [ ] 发送包含身份证号的消息，显示 Toast 警告
- [ ] Toast 显示正确的脱敏数量
- [ ] Toast 3秒后自动消失
- [ ] 状态指示器显示 DLP 启用状态
- [ ] 状态指示器样式正确（绿色/灰色）

### 用户体验验收
- [ ] Toast 不遮挡聊天内容
- [ ] 状态指示器不影响输入框布局
- [ ] 动画流畅自然
- [ ] 颜色符合设计规范

### 技术验收
- [ ] 无 TypeScript 错误
- [ ] 无 React 警告
- [ ] 组件可复用
- [ ] 代码简洁清晰

## 样式参考

### Toast 动画

```css
@keyframes slide-in {
  from {
    transform: translateX(100%);
    opacity: 0;
  }
  to {
    transform: translateX(0);
    opacity: 1;
  }
}

.animate-slide-in {
  animation: slide-in 0.3s ease-out;
}
```

### 状态指示器样式

```css
/* 启用状态 */
.dlp-enabled {
  background-color: rgba(34, 197, 94, 0.1);
  color: rgb(34, 197, 94);
}

/* 禁用状态 */
.dlp-disabled {
  background-color: rgba(156, 163, 175, 0.1);
  color: rgb(156, 163, 175);
}
```

## 测试场景

### 场景 1：脱敏提示
1. 输入：`我的手机号是 13800138000`
2. 点击发送
3. 预期：显示 Toast "已脱敏 1 处敏感信息"
4. 预期：消息显示为 `138*****000`

### 场景 2：阻止提示
1. 输入：`阿里云密钥：LTAI4G8aB9cD2eFgH3iJ`
2. 点击发送
3. 预期：显示错误提示
4. 预期：消息未被发送

### 场景 3：状态指示器
1. 打开聊天界面
2. 预期：看到 🛡️ DLP 已启用（绿色）
3. 鼠标悬停
4. 预期：显示提示 "DLP 数据保护已启用"

## 完成后的效果

### 用户视角

```
┌─────────────────────────────────────────┐
│  聊天界面                    [Toast 警告] │
│                                          │
│  用户: 我的手机号是 138*****000          │
│  助手: 好的，我记下了                    │
│                                          │
│  ┌────────────────────────────────────┐ │
│  │ 输入消息...        🛡️ DLP 已启用  │ │
│  └────────────────────────────────────┘ │
└─────────────────────────────────────────┘

[Toast 警告]
┌──────────────────────────┐
│ ⚠️  检测到敏感信息        │
│ 已脱敏 1 处敏感信息       │
│                      [×] │
└──────────────────────────┘
```

### 用户体验流程

```
用户输入消息
    ↓
DLP 自动扫描（< 10ms）
    ↓
检测到敏感信息
    ↓
显示 Toast 警告（3秒）
    ↓
使用脱敏内容发送
    ↓
用户看到脱敏后的消息
```

## 后续优化（可选）

### P1 优化（1-2天）
- [ ] 点击 Toast 查看详情
- [ ] 点击状态指示器打开配置
- [ ] 添加"发送原文"选项（需二次确认）
- [ ] 添加脱敏历史记录

### P2 优化（3-5天）
- [ ] 实时高亮敏感信息（输入时）
- [ ] 自定义规则管理界面
- [ ] 统计图表展示
- [ ] 审计日志查看器

## 工作量估算

| 任务 | 预计时间 | 实际时间 |
|------|---------|---------|
| 创建 Toast 组件 | 30分钟 | ✅ 已完成 |
| 创建状态指示器 | 30分钟 | ✅ 已完成 |
| 集成到聊天界面 | 45分钟 | 待完成 |
| 集成到输入框 | 45分钟 | 待完成 |
| 测试和调整 | 30分钟 | 待完成 |
| **总计** | **3小时** | **1小时已完成** |

## 下一步

1. 找到聊天界面组件（ChatTab.tsx 或类似）
2. 集成 DlpWarningToast
3. 集成 DlpStatusIndicator
4. 测试和调整样式
5. 运行 E2E 测试验证

## 参考

- 组件已创建：
  - `DlpWarningToast.tsx` ✅
  - `DlpStatusIndicator.tsx` ✅
- Hook 已更新：
  - `useAiChat.ts` ✅（已添加 dlpWarning 状态）
- 测试已就绪：
  - `cypress/e2e/dlp_integration.cy.js` ✅
