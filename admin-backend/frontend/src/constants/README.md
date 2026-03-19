# 常量定义说明

本目录包含前端应用中使用的常量定义，用于统一管理和复用。

## DLP 常量 (`dlp.ts`)

### 使用场景

1. **创建/编辑 DLP 规则表单**
2. **DLP 规则列表展示**
3. **DLP 规则筛选器**
4. **DLP 规则统计图表**
5. **任何需要显示或选择 DLP 分类/严重级别的地方**

### 使用示例

#### 1. 在表单中使用分类选项

```tsx
import { DLP_CATEGORY_OPTIONS } from '../../constants/dlp';
import { Select } from 'antd';

const { Option } = Select;

function MyForm() {
  return (
    <Select placeholder="请选择分类">
      {DLP_CATEGORY_OPTIONS.map(option => (
        <Option key={option.value} value={option.value}>
          {option.label}
        </Option>
      ))}
    </Select>
  );
}
```

#### 2. 在表单中使用严重级别选项

```tsx
import { DLP_SEVERITY_OPTIONS } from '../../constants/dlp';
import { Select } from 'antd';

const { Option } = Select;

function MyForm() {
  return (
    <Select placeholder="请选择严重级别">
      {DLP_SEVERITY_OPTIONS.map(option => (
        <Option key={option.value} value={option.value}>
          {option.label}
        </Option>
      ))}
    </Select>
  );
}
```

#### 3. 在列表中显示分类文本

```tsx
import { getCategoryText } from '../../constants/dlp';

function RuleList({ rules }) {
  return (
    <ul>
      {rules.map(rule => (
        <li key={rule.id}>
          {rule.name} - {getCategoryText(rule.category)}
        </li>
      ))}
    </ul>
  );
}
```

#### 4. 在列表中显示严重级别标签

```tsx
import { getSeverityText, getSeverityColor } from '../../constants/dlp';
import { Tag } from 'antd';

function RuleList({ rules }) {
  return (
    <ul>
      {rules.map(rule => (
        <li key={rule.id}>
          {rule.name} - 
          <Tag color={getSeverityColor(rule.severity)}>
            {getSeverityText(rule.severity)}
          </Tag>
        </li>
      ))}
    </ul>
  );
}
```

#### 5. 在筛选器中使用

```tsx
import { DLP_CATEGORY_OPTIONS, DLP_SEVERITY_OPTIONS } from '../../constants/dlp';
import { Select, Space } from 'antd';

const { Option } = Select;

function RuleFilter({ onFilterChange }) {
  return (
    <Space>
      <Select 
        placeholder="筛选分类" 
        onChange={(value) => onFilterChange('category', value)}
        allowClear
      >
        {DLP_CATEGORY_OPTIONS.map(option => (
          <Option key={option.value} value={option.value}>
            {option.label}
          </Option>
        ))}
      </Select>
      
      <Select 
        placeholder="筛选严重级别" 
        onChange={(value) => onFilterChange('severity', value)}
        allowClear
      >
        {DLP_SEVERITY_OPTIONS.map(option => (
          <Option key={option.value} value={option.value}>
            {option.label}
          </Option>
        ))}
      </Select>
    </Space>
  );
}
```

#### 6. 在统计图表中使用

```tsx
import { CATEGORY_MAP, SEVERITY_MAP } from '../../constants/dlp';
import { Pie } from '@ant-design/charts';

function RuleStatistics({ rules }) {
  // 按分类统计
  const categoryData = Object.entries(CATEGORY_MAP).map(([value, label]) => ({
    type: label,
    value: rules.filter(r => r.category === value).length,
  }));

  // 按严重级别统计
  const severityData = Object.entries(SEVERITY_MAP).map(([value, { label }]) => ({
    type: label,
    value: rules.filter(r => r.severity === value).length,
  }));

  return (
    <div>
      <h3>按分类统计</h3>
      <Pie data={categoryData} angleField="value" colorField="type" />
      
      <h3>按严重级别统计</h3>
      <Pie data={severityData} angleField="value" colorField="type" />
    </div>
  );
}
```

#### 7. 在 Tooltip 中显示详细信息

```tsx
import { DLP_CATEGORY_OPTIONS } from '../../constants/dlp';
import { Tooltip } from 'antd';

function RuleItem({ rule }) {
  const categoryOption = DLP_CATEGORY_OPTIONS.find(
    opt => opt.value === rule.category
  );

  return (
    <Tooltip title={categoryOption?.description}>
      <span>{categoryOption?.label}</span>
    </Tooltip>
  );
}
```

### 可用常量

#### `DLP_SEVERITY_OPTIONS`
严重级别选项数组，包含 value、label、color 字段。

#### `DLP_CATEGORY_OPTIONS`
分类选项数组，包含 value、label、description 字段。

#### `SEVERITY_MAP`
严重级别映射对象，key 为 value，value 为 { label, color }。

#### `CATEGORY_MAP`
分类映射对象，key 为 value，value 为 label。

### 工具函数

#### `getSeverityText(severity: string): string`
获取严重级别的中文文本。

#### `getSeverityColor(severity: string): string`
获取严重级别的颜色（用于 Tag 组件）。

#### `getCategoryText(category: string): string`
获取分类的中文文本。

## 添加新常量

如果需要添加新的常量定义：

1. 在 `constants/` 目录下创建新的 `.ts` 文件
2. 导出常量、类型和工具函数
3. 在本 README 中添加使用说明
4. 更新相关组件使用新常量

## 注意事项

1. **不要在组件中硬编码选项** - 始终使用常量文件中的定义
2. **保持一致性** - 所有使用相同数据的地方都应该引用同一个常量
3. **类型安全** - 使用 TypeScript 的 `as const` 确保类型推断正确
4. **文档更新** - 修改常量时记得更新本 README
