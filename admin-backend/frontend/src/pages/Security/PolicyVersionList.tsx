import React, { useState, useEffect, useCallback } from 'react';
import {
  Button, Tag, Input, Select, DatePicker, Empty, Timeline, Card, Row, Col,
  Statistic, Pagination, Spin, message, Tooltip,
} from 'antd';
import {
  ReloadOutlined,
  SearchOutlined,
  HistoryOutlined,
  UserOutlined,
  ClockCircleOutlined,
  FileTextOutlined,
  ArrowRightOutlined,
  PlusCircleOutlined,
  EditOutlined,
  DeleteOutlined,
  CheckCircleOutlined,
  StopOutlined,
  ImportOutlined,
} from '@ant-design/icons';
import { apiClient } from '../../api/client';
import type { PolicyChangeRecord, PolicyChangeStats } from '../../types';
import {
  getChangeTypeText,
  getChangeTypeColor,
  getRuleTypeText,
  getRuleTypeColor,
  getFieldLabel,
  formatChangeValue,
  CHANGE_TYPE_OPTIONS,
  RULE_TYPE_OPTIONS,
} from '../../constants/policyChanges';
import '../../styles/PolicyVersionList.css';

const { Option } = Select;
const { RangePicker } = DatePicker;

/** 变更类型对应的图标 */
function getChangeTypeIcon(type: string): React.ReactNode {
  const iconMap: Record<string, React.ReactNode> = {
    create: <PlusCircleOutlined style={{ color: '#52c41a' }} />,
    update: <EditOutlined style={{ color: '#1890ff' }} />,
    delete: <DeleteOutlined style={{ color: '#ff4d4f' }} />,
    enable: <CheckCircleOutlined style={{ color: '#13c2c2' }} />,
    disable: <StopOutlined style={{ color: '#faad14' }} />,
    import: <ImportOutlined style={{ color: '#722ed1' }} />,
  };
  return iconMap[type] || <FileTextOutlined />;
}

/** 变更类型对应的时间线颜色 */
function getTimelineDotColor(type: string): string {
  const colorMap: Record<string, string> = {
    create: 'green',
    update: 'blue',
    delete: 'red',
    enable: 'cyan',
    disable: 'orange',
    import: 'purple',
  };
  return colorMap[type] || 'gray';
}

/** Diff 展示组件 */
const DiffDisplay: React.FC<{ record: PolicyChangeRecord }> = ({ record }) => {
  if (record.change_type === 'create' && record.new_value) {
    return (
      <div className="policy-change-diff">
        <div className="diff-field-label">创建内容</div>
        <div className="diff-new" style={{ maxWidth: '100%' }}>
          {typeof record.new_value === 'object'
            ? Object.entries(record.new_value)
                .filter(([, v]) => v !== null && v !== undefined)
                .map(([k, v]) => `${getFieldLabel(k)}: ${formatChangeValue(v)}`)
                .join('；')
            : formatChangeValue(record.new_value)}
        </div>
      </div>
    );
  }

  if (record.change_type === 'delete' && record.old_value) {
    return (
      <div className="policy-change-diff">
        <div className="diff-field-label">删除内容</div>
        <div className="diff-old" style={{ maxWidth: '100%', textDecoration: 'line-through' }}>
          {typeof record.old_value === 'object'
            ? Object.entries(record.old_value)
                .filter(([, v]) => v !== null && v !== undefined)
                .map(([k, v]) => `${getFieldLabel(k)}: ${formatChangeValue(v)}`)
                .join('；')
            : formatChangeValue(record.old_value)}
        </div>
      </div>
    );
  }

  if (record.change_type === 'update') {
    return (
      <div className="policy-change-diff">
        {record.field_changed && (
          <div className="diff-field-label">变更字段：{getFieldLabel(record.field_changed)}</div>
        )}
        <div className="diff-row">
          <div className="diff-old">{formatChangeValue(record.old_value)}</div>
          <span className="diff-arrow"><ArrowRightOutlined /></span>
          <div className="diff-new">{formatChangeValue(record.new_value)}</div>
        </div>
      </div>
    );
  }

  if (record.change_type === 'enable' || record.change_type === 'disable') {
    return (
      <div className="policy-change-diff">
        <div className="diff-row">
          <Tag color={record.change_type === 'enable' ? 'success' : 'warning'}>
            {record.change_type === 'enable' ? '已启用' : '已禁用'}
          </Tag>
        </div>
      </div>
    );
  }

  return null;
};

/** 变更记录卡片 */
const ChangeCard: React.FC<{ record: PolicyChangeRecord }> = ({ record }) => {
  return (
    <div className="policy-change-card">
      <div className="policy-change-header">
        <span className="policy-change-rule-name">
          {record.rule_name || record.rule_id.slice(0, 8)}
        </span>
        <Tag color={getRuleTypeColor(record.rule_type)}>
          {getRuleTypeText(record.rule_type)}
        </Tag>
        <Tag color={getChangeTypeColor(record.change_type)}>
          {getChangeTypeText(record.change_type)}
        </Tag>
      </div>
      <div className="policy-change-meta">
        <Tooltip title="操作人">
          <span className="meta-item">
            <UserOutlined /> {record.changed_by_name || 'system'}
          </span>
        </Tooltip>
        <Tooltip title="操作时间">
          <span className="meta-item">
            <ClockCircleOutlined /> {formatDateTime(record.changed_at)}
          </span>
        </Tooltip>
        {record.reason && (
          <Tooltip title="变更原因">
            <span className="meta-item">
              <FileTextOutlined /> {record.reason}
            </span>
          </Tooltip>
        )}
      </div>
      <DiffDisplay record={record} />
    </div>
  );
};

/** 主页面组件 */
export const PolicyVersionList: React.FC = () => {
  const [records, setRecords] = useState<PolicyChangeRecord[]>([]);
  const [stats, setStats] = useState<PolicyChangeStats | null>(null);
  const [loading, setLoading] = useState(false);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);

  // 筛选状态
  const [searchText, setSearchText] = useState('');
  const [ruleTypeFilter, setRuleTypeFilter] = useState<string | undefined>(undefined);
  const [changeTypeFilter, setChangeTypeFilter] = useState<string | undefined>(undefined);
  const [dateRange, setDateRange] = useState<[any, any] | null>(null);

  const loadRecords = useCallback(async () => {
    setLoading(true);
    try {
      const params: Record<string, any> = { page, page_size: pageSize };
      if (searchText) params.search = searchText;
      if (ruleTypeFilter) params.rule_type = ruleTypeFilter;
      if (changeTypeFilter) params.change_type = changeTypeFilter;
      if (dateRange && dateRange[0] && dateRange[1]) {
        params.start_date = dateRange[0].format('YYYY-MM-DD');
        params.end_date = dateRange[1].format('YYYY-MM-DD');
      }

      const response = await apiClient.get('/policy-changes', { params });
      setRecords(response.data.records || []);
      setTotal(response.data.total || 0);
    } catch (error: any) {
      message.error(error.response?.data?.error || '加载变更记录失败');
    } finally {
      setLoading(false);
    }
  }, [page, pageSize, searchText, ruleTypeFilter, changeTypeFilter, dateRange]);

  const loadStats = useCallback(async () => {
    try {
      const response = await apiClient.get('/policy-changes/stats');
      setStats(response.data);
    } catch {
      // 统计加载失败不阻塞主流程
    }
  }, []);

  useEffect(() => {
    loadRecords();
  }, [loadRecords]);

  useEffect(() => {
    loadStats();
  }, [loadStats]);

  const hasActiveFilters = searchText || ruleTypeFilter || changeTypeFilter || dateRange;

  const clearFilters = useCallback(() => {
    setSearchText('');
    setRuleTypeFilter(undefined);
    setChangeTypeFilter(undefined);
    setDateRange(null);
    setPage(1);
  }, []);

  const handlePageChange = useCallback((newPage: number, newPageSize: number) => {
    setPage(newPage);
    setPageSize(newPageSize);
  }, []);

  // 搜索防抖：回车或清空时触发
  const handleSearchChange = useCallback((value: string) => {
    setSearchText(value);
    setPage(1);
  }, []);

  return (
    <div className="policy-version-list-container">
      <div className="page-header">
        <h2>策略变更记录</h2>
        <Button icon={<ReloadOutlined />} onClick={() => { loadRecords(); loadStats(); }} loading={loading}>
          刷新
        </Button>
      </div>

      {/* 统计卡片 */}
      {stats && (
        <Row gutter={16} className="policy-stats-row">
          <Col span={6}>
            <Card size="small" className="policy-stat-card">
              <Statistic title="总变更次数" value={stats.total} prefix={<HistoryOutlined />} />
            </Card>
          </Col>
          {stats.by_change_type.slice(0, 3).map((item) => (
            <Col span={6} key={item.type}>
              <Card size="small" className="policy-stat-card">
                <Statistic
                  title={`${getChangeTypeText(item.type)}操作`}
                  value={item.count}
                  prefix={getChangeTypeIcon(item.type)}
                />
              </Card>
            </Col>
          ))}
        </Row>
      )}

      {/* 筛选栏 */}
      <div className="policy-filter-bar">
        <Input
          placeholder="搜索规则名称或操作人"
          prefix={<SearchOutlined />}
          value={searchText}
          onChange={(e) => handleSearchChange(e.target.value)}
          allowClear
          style={{ width: 240 }}
        />
        <Select
          placeholder="规则类型"
          value={ruleTypeFilter}
          onChange={(v) => { setRuleTypeFilter(v); setPage(1); }}
          allowClear
          style={{ width: 140 }}
        >
          {RULE_TYPE_OPTIONS.map((opt) => (
            <Option key={opt.value} value={opt.value}>
              <Tag color={opt.color} style={{ marginRight: 4 }}>{opt.label}</Tag>
            </Option>
          ))}
        </Select>
        <Select
          placeholder="变更类型"
          value={changeTypeFilter}
          onChange={(v) => { setChangeTypeFilter(v); setPage(1); }}
          allowClear
          style={{ width: 130 }}
        >
          {CHANGE_TYPE_OPTIONS.map((opt) => (
            <Option key={opt.value} value={opt.value}>
              <Tag color={opt.color} style={{ marginRight: 4 }}>{opt.label}</Tag>
            </Option>
          ))}
        </Select>
        <RangePicker
          value={dateRange as any}
          onChange={(dates) => { setDateRange(dates as any); setPage(1); }}
          placeholder={['开始日期', '结束日期']}
          style={{ width: 240 }}
        />
        {hasActiveFilters && (
          <Button type="link" onClick={clearFilters} size="small">
            清除筛选
          </Button>
        )}
        <span className="policy-filter-summary">
          共 {total} 条记录
        </span>
      </div>

      {/* 时间线内容 */}
      <Spin spinning={loading}>
        {records.length > 0 ? (
          <div className="policy-timeline-container">
            <Timeline
              items={records.map((record) => ({
                color: getTimelineDotColor(record.change_type),
                icon: getChangeTypeIcon(record.change_type),
                content: <ChangeCard record={record} />,
              }))}
            />
          </div>
        ) : !loading ? (
          <div className="policy-empty-state">
            <Empty
              image={<HistoryOutlined style={{ fontSize: 48, color: '#bfbfbf' }} />}
              description={
                hasActiveFilters ? (
                  <span>
                    没有匹配的变更记录，
                    <Button type="link" onClick={clearFilters} style={{ padding: 0 }}>清除筛选条件</Button>
                  </span>
                ) : (
                  '暂无策略变更记录，当 DLP 规则、敏感操作或字典发生变更时，记录将自动生成'
                )
              }
            />
          </div>
        ) : null}
      </Spin>

      {/* 分页 */}
      {total > pageSize && (
        <div className="policy-pagination">
          <Pagination
            current={page}
            pageSize={pageSize}
            total={total}
            onChange={handlePageChange}
            showSizeChanger
            showTotal={(t) => `共 ${t} 条记录`}
            pageSizeOptions={['10', '20', '50']}
          />
        </div>
      )}
    </div>
  );
};

/** 格式化日期时间 */
function formatDateTime(dateStr: string): string {
  const date = new Date(dateStr);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMin = Math.floor(diffMs / 60000);
  const diffHour = Math.floor(diffMs / 3600000);
  const diffDay = Math.floor(diffMs / 86400000);

  if (diffMin < 1) return '刚刚';
  if (diffMin < 60) return `${diffMin} 分钟前`;
  if (diffHour < 24) return `${diffHour} 小时前`;
  if (diffDay < 3) return `${diffDay} 天前`;
  return date.toLocaleString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });
}
