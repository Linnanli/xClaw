import React, { useState, useEffect, useMemo, useCallback } from 'react';
import { Table, Button, Space, message, Tag, Input, Select, DatePicker, Empty, Modal, Descriptions } from 'antd';
import {
  ReloadOutlined,
  SearchOutlined,
  AuditOutlined,
  ExportOutlined,
  EyeOutlined,
} from '@ant-design/icons';
import type { ColumnsType, TablePaginationConfig } from 'antd/es/table';
import dayjs, { type Dayjs } from 'dayjs';
import { apiClient } from '../api/client';
import '../styles/AuditLog.css';

const { Option } = Select;
const { RangePicker } = DatePicker;

/** 审计日志条目 */
interface AuditLogEntry {
  id: string;
  user_id: string | null;
  username: string | null;
  action: string;
  details: string;
  created_at: string;
}

/** 操作类型 → 中文映射 */
const ACTION_LABEL_MAP: Record<string, string> = {
  create_dlp_rule: '创建 DLP 规则',
  update_dlp_rule: '更新 DLP 规则',
  delete_dlp_rule: '删除 DLP 规则',
  toggle_dlp_rule: '切换 DLP 规则状态',
  batch_enable_dlp_rules: '批量启用 DLP 规则',
  batch_disable_dlp_rules: '批量禁用 DLP 规则',
  batch_delete_dlp_rules: '批量删除 DLP 规则',
  import_dlp_rules: '导入 DLP 规则',
  export_dlp_rules: '导出 DLP 规则',
  create_dictionary: '创建字典',
  update_dictionary: '更新字典',
  delete_dictionary: '删除字典',
  create_user: '创建用户',
  update_user: '更新用户',
  delete_user: '删除用户',
  login: '用户登录',
  logout: '用户登出',
  create_role: '创建角色',
  update_role: '更新角色',
  delete_role: '删除角色',
  assign_permissions: '分配权限',
  assign_roles: '分配角色',
  dlp_scan: 'DLP 扫描',
  dlp_block: 'DLP 拦截',
};

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
  toggle: 'blue',
  batch: 'purple',
  scan: 'cyan',
  block: 'red',
  assign: 'blue',
};

/** 获取操作类型中文标签 */
function getActionLabel(action: string): string {
  return ACTION_LABEL_MAP[action] || action;
}

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
  const [total, setTotal] = useState(0);
  const [currentPage, setCurrentPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);

  // 筛选状态
  const [searchText, setSearchText] = useState('');
  const [actionFilter, setActionFilter] = useState<string | undefined>(undefined);
  const [dateRange, setDateRange] = useState<[Dayjs | null, Dayjs | null] | null>(null);

  // 详情弹窗
  const [detailVisible, setDetailVisible] = useState(false);
  const [detailLog, setDetailLog] = useState<AuditLogEntry | null>(null);

  const loadLogs = useCallback(async (page = currentPage, size = pageSize) => {
    setLoading(true);
    try {
      const response = await apiClient.get('/audit-logs', {
        params: { page, page_size: size },
      });
      setLogs(response.data.logs || []);
      setTotal(response.data.total || 0);
    } catch (error: any) {
      message.error(error.response?.data?.error || '加载审计日志失败');
    } finally {
      setLoading(false);
    }
  }, [currentPage, pageSize]);

  useEffect(() => {
    loadLogs(currentPage, pageSize);
  }, [currentPage, pageSize]); // eslint-disable-line react-hooks/exhaustive-deps

  // 提取所有不重复的 action 值用于筛选下拉
  const actionOptions = useMemo(() => {
    const set = new Set(logs.map((l) => l.action));
    return Array.from(set).sort();
  }, [logs]);

  // 前端筛选（在当前页数据上筛选）
  const filteredLogs = useMemo(() => {
    return logs.filter((log) => {
      if (searchText) {
        const kw = searchText.toLowerCase();
        const match =
          log.action.toLowerCase().includes(kw) ||
          getActionLabel(log.action).toLowerCase().includes(kw) ||
          log.details.toLowerCase().includes(kw) ||
          (log.username || '').toLowerCase().includes(kw) ||
          (log.user_id || '').toLowerCase().includes(kw);
        if (!match) return false;
      }
      if (actionFilter && log.action !== actionFilter) return false;
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

  // 查看详情
  const handleViewDetail = useCallback((log: AuditLogEntry) => {
    setDetailLog(log);
    setDetailVisible(true);
  }, []);

  // 导出（使用后端 API）
  const handleExport = useCallback(async () => {
    try {
      const params: Record<string, string> = {};
      if (dateRange && dateRange[0]) params.start_time = dateRange[0].toISOString();
      if (dateRange && dateRange[1]) params.end_time = dateRange[1].toISOString();
      if (actionFilter) params.action = actionFilter;
      if (searchText) params.username = searchText;

      const response = await apiClient.get('/audit-logs/export', {
        params,
        responseType: 'blob',
      });

      const blob = new Blob([response.data], { type: 'text/csv;charset=utf-8' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `audit-logs-${dayjs().format('YYYY-MM-DD')}.csv`;
      a.click();
      URL.revokeObjectURL(url);
      message.success('导出成功');
    } catch {
      message.error('导出失败');
    }
  }, [dateRange, actionFilter, searchText]);

  // 分页变化
  const handleTableChange = useCallback((pagination: TablePaginationConfig) => {
    if (pagination.current) setCurrentPage(pagination.current);
    if (pagination.pageSize) setPageSize(pagination.pageSize);
  }, []);

  /** 渲染操作人 */
  const renderUser = (log: AuditLogEntry) => {
    if (log.username) {
      return <span style={{ fontWeight: 500 }}>{log.username}</span>;
    }
    if (!log.user_id || log.user_id === '00000000-0000-0000-0000-000000000000') {
      return <Tag color="default">系统</Tag>;
    }
    return <span style={{ fontFamily: 'monospace', fontSize: 12, color: '#8c8c8c' }}>{log.user_id.slice(0, 8)}...</span>;
  };

  const columns: ColumnsType<AuditLogEntry> = [
    {
      title: '时间',
      dataIndex: 'created_at',
      key: 'created_at',
      width: 180,
      render: (date: string) => formatTime(date),
    },
    {
      title: '操作人',
      key: 'username',
      width: 120,
      render: (_, record) => renderUser(record),
    },
    {
      title: '操作类型',
      dataIndex: 'action',
      key: 'action',
      width: 180,
      render: (action: string) => (
        <Tag color={getActionColor(action)} className="audit-action-tag">
          {getActionLabel(action)}
        </Tag>
      ),
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
    {
      title: '操作',
      key: 'action_col',
      width: 80,
      render: (_, record) => (
        <Button type="link" size="small" icon={<EyeOutlined />} onClick={() => handleViewDetail(record)}>
          详情
        </Button>
      ),
    },
  ];

  const emptyContent = logs.length === 0 && !loading ? (
    <Empty
      image={<AuditOutlined style={{ fontSize: 48, color: '#bfbfbf' }} />}
      description="暂无审计日志"
    />
  ) : hasActiveFilters && filteredLogs.length === 0 ? (
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
            导出 CSV
          </Button>
          <Button icon={<ReloadOutlined />} onClick={() => loadLogs(currentPage, pageSize)} loading={loading}>
            刷新
          </Button>
        </Space>
      </div>

      {/* 筛选栏 */}
      <div className="audit-filter-bar">
        <Input
          placeholder="搜索操作人、操作类型或详情"
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
          style={{ width: 180 }}
        >
          {actionOptions.map((action) => (
            <Option key={action} value={action}>
              {getActionLabel(action)}
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
        <span className="audit-filter-summary">
          共 {total} 条日志
          {hasActiveFilters && filteredLogs.length !== logs.length && `，当前页显示 ${filteredLogs.length} 条`}
        </span>
      </div>

      <Table
        columns={columns}
        dataSource={filteredLogs}
        rowKey="id"
        loading={loading}
        locale={{ emptyText: emptyContent }}
        pagination={{
          current: currentPage,
          pageSize: pageSize,
          total: hasActiveFilters ? filteredLogs.length : total,
          showSizeChanger: true,
          pageSizeOptions: ['20', '50', '100'],
          showTotal: (t) => `共 ${t} 条日志`,
        }}
        onChange={handleTableChange}
        scroll={{ x: 900 }}
        size="middle"
      />

      {/* 详情弹窗 */}
      <Modal
        title="审计日志详情"
        open={detailVisible}
        onCancel={() => setDetailVisible(false)}
        footer={<Button onClick={() => setDetailVisible(false)}>关闭</Button>}
        width={640}
      >
        {detailLog && (
          <Descriptions column={1} bordered size="small">
            <Descriptions.Item label="时间">{formatTime(detailLog.created_at)}</Descriptions.Item>
            <Descriptions.Item label="操作人">
              {detailLog.username || (detailLog.user_id ? detailLog.user_id : '系统')}
            </Descriptions.Item>
            <Descriptions.Item label="操作类型">
              <Tag color={getActionColor(detailLog.action)}>
                {getActionLabel(detailLog.action)}
              </Tag>
              <span style={{ marginLeft: 8, color: '#8c8c8c', fontSize: 12 }}>({detailLog.action})</span>
            </Descriptions.Item>
            <Descriptions.Item label="详情">
              <div style={{ whiteSpace: 'pre-wrap', wordBreak: 'break-all', maxHeight: 300, overflow: 'auto' }}>
                {detailLog.details}
              </div>
            </Descriptions.Item>
            <Descriptions.Item label="日志 ID">
              <span style={{ fontFamily: 'monospace', fontSize: 12 }}>{detailLog.id}</span>
            </Descriptions.Item>
          </Descriptions>
        )}
      </Modal>
    </div>
  );
};
