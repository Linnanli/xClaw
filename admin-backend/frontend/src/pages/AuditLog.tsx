import React, { useState, useEffect, useMemo, useCallback } from 'react';
import { Table, Button, Space, message, Tag, Input, Select, DatePicker, Empty } from 'antd';
import {
  ReloadOutlined,
  SearchOutlined,
  AuditOutlined,
  ExportOutlined,
} from '@ant-design/icons';
import type { ColumnsType } from 'antd/es/table';
import dayjs, { type Dayjs } from 'dayjs';
import { apiClient } from '../api/client';
import '../styles/AuditLog.css';

const { Option } = Select;
const { RangePicker } = DatePicker;

/** 审计日志条目（匹配后端 GET /api/audit-logs 返回格式） */
interface AuditLogEntry {
  id: string;
  user_id: string;
  action: string;
  details: string;
  created_at: string;
}

/** 操作类型 → 颜色映射 */
const ACTION_COLOR_MAP: Record<string, string> = {
  create: 'green',
  update: 'blue',
  delete: 'red',
  login: 'cyan',
  logout: 'default',
  enable: 'green',
  disable: 'orange',
  import: 'purple',
  export: 'geekblue',
};

/** 根据 action 字符串推断颜色 */
function getActionColor(action: string): string {
  const lower = action.toLowerCase();
  for (const [key, color] of Object.entries(ACTION_COLOR_MAP)) {
    if (lower.includes(key)) return color;
  }
  return 'default';
}

/** 格式化时间 */
function formatTime(dateStr: string): string {
  const d = new Date(dateStr);
  return d.toLocaleString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  });
}

export const AuditLog: React.FC = () => {
  const [logs, setLogs] = useState<AuditLogEntry[]>([]);
  const [loading, setLoading] = useState(false);

  // 筛选状态
  const [searchText, setSearchText] = useState('');
  const [actionFilter, setActionFilter] = useState<string | undefined>(undefined);
  const [dateRange, setDateRange] = useState<[Dayjs | null, Dayjs | null] | null>(null);

  const loadLogs = useCallback(async () => {
    setLoading(true);
    try {
      const response = await apiClient.get('/audit-logs');
      setLogs(response.data.logs || []);
    } catch (error: any) {
      message.error(error.response?.data?.error || '加载审计日志失败');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadLogs();
  }, [loadLogs]);

  // 提取所有不重复的 action 值用于筛选下拉
  const actionOptions = useMemo(() => {
    const set = new Set(logs.map((l) => l.action));
    return Array.from(set).sort();
  }, [logs]);

  // 前端筛选
  const filteredLogs = useMemo(() => {
    return logs.filter((log) => {
      // 文本搜索
      if (searchText) {
        const kw = searchText.toLowerCase();
        const match =
          log.action.toLowerCase().includes(kw) ||
          log.details.toLowerCase().includes(kw) ||
          log.user_id.toLowerCase().includes(kw);
        if (!match) return false;
      }
      // 操作类型筛选
      if (actionFilter && log.action !== actionFilter) return false;
      // 日期范围筛选
      if (dateRange && dateRange[0] && dateRange[1]) {
        const logDate = dayjs(log.created_at);
        if (logDate.isBefore(dateRange[0], 'day') || logDate.isAfter(dateRange[1], 'day')) {
          return false;
        }
      }
      return true;
    });
  }, [logs, searchText, actionFilter, dateRange]);

  const hasActiveFilters = searchText || actionFilter || dateRange;

  const clearFilters = useCallback(() => {
    setSearchText('');
    setActionFilter(undefined);
    setDateRange(null);
  }, []);

  // 导出为 JSON
  const handleExport = useCallback(() => {
    const data = JSON.stringify(filteredLogs, null, 2);
    const blob = new Blob([data], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `audit-logs-${dayjs().format('YYYY-MM-DD')}.json`;
    a.click();
    URL.revokeObjectURL(url);
    message.success('导出成功');
  }, [filteredLogs]);

  const columns: ColumnsType<AuditLogEntry> = [
    {
      title: '时间',
      dataIndex: 'created_at',
      key: 'created_at',
      width: 180,
      sorter: (a, b) => new Date(a.created_at).getTime() - new Date(b.created_at).getTime(),
      defaultSortOrder: 'descend',
      render: (date: string) => formatTime(date),
    },
    {
      title: '操作类型',
      dataIndex: 'action',
      key: 'action',
      width: 160,
      render: (action: string) => (
        <Tag color={getActionColor(action)} className="audit-action-tag">
          {action}
        </Tag>
      ),
    },
    {
      title: '用户 ID',
      dataIndex: 'user_id',
      key: 'user_id',
      width: 280,
      ellipsis: true,
      render: (uid: string) => {
        if (!uid || uid === '00000000-0000-0000-0000-000000000000') {
          return <Tag color="default">系统 / 客户端</Tag>;
        }
        return <span style={{ fontFamily: 'monospace', fontSize: 12 }}>{uid}</span>;
      },
    },
    {
      title: '详情',
      dataIndex: 'details',
      key: 'details',
      ellipsis: true,
      render: (details: string) => (
        <span className="audit-details-cell">{details}</span>
      ),
    },
  ];

  const emptyContent = logs.length === 0 ? (
    <Empty
      image={<AuditOutlined style={{ fontSize: 48, color: '#bfbfbf' }} />}
      description="暂无审计日志"
    />
  ) : hasActiveFilters ? (
    <Empty
      description={
        <span>
          没有匹配的日志，<Button type="link" onClick={clearFilters} style={{ padding: 0 }}>清除筛选条件</Button>
        </span>
      }
    />
  ) : undefined;

  return (
    <div className="audit-log-container">
      <div className="page-header">
        <h2>审计日志</h2>
        <Space>
          <Button icon={<ExportOutlined />} onClick={handleExport} disabled={filteredLogs.length === 0}>
            导出
          </Button>
          <Button icon={<ReloadOutlined />} onClick={loadLogs} loading={loading}>
            刷新
          </Button>
        </Space>
      </div>

      {/* 筛选栏 */}
      <div className="audit-filter-bar">
        <Input
          placeholder="搜索操作类型、详情或用户 ID"
          prefix={<SearchOutlined />}
          value={searchText}
          onChange={(e) => setSearchText(e.target.value)}
          allowClear
          style={{ width: 280 }}
        />
        <Select
          placeholder="操作类型"
          value={actionFilter}
          onChange={setActionFilter}
          allowClear
          style={{ width: 160 }}
        >
          {actionOptions.map((action) => (
            <Option key={action} value={action}>
              {action}
            </Option>
          ))}
        </Select>
        <RangePicker
          value={dateRange as any}
          onChange={(dates) => setDateRange(dates as [Dayjs | null, Dayjs | null] | null)}
          placeholder={['开始日期', '结束日期']}
        />
        {hasActiveFilters && (
          <Button type="link" onClick={clearFilters} size="small">
            清除筛选
          </Button>
        )}
        {logs.length > 0 && (
          <span className="audit-filter-summary">
            共 {logs.length} 条日志{filteredLogs.length !== logs.length && `，显示 ${filteredLogs.length} 条`}
          </span>
        )}
      </div>

      <Table
        columns={columns}
        dataSource={filteredLogs}
        rowKey="id"
        loading={loading}
        locale={{ emptyText: emptyContent }}
        pagination={filteredLogs.length > 20 ? {
          pageSize: 20,
          showSizeChanger: true,
          pageSizeOptions: ['20', '50', '100'],
          showTotal: (total) => `共 ${total} 条日志`,
        } : false}
        scroll={{ x: 900 }}
        size="middle"
      />
    </div>
  );
};
