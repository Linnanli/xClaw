import React, { useState, useEffect, useMemo, useCallback } from 'react';
import { Table, Button, Space, message, Popconfirm, Tag, Input, Select, Empty, Card, Row, Col, Statistic, Tooltip } from 'antd';
import {
  ReloadOutlined,
  SearchOutlined,
  DeleteOutlined,
  DesktopOutlined,
  AppleOutlined,
  WindowsOutlined,
  CloudServerOutlined,
  CheckCircleOutlined,
  CloseCircleOutlined,
} from '@ant-design/icons';
import type { ColumnsType } from 'antd/es/table';
import { apiClient } from '../api/client';
import type { Client, ClientStats } from '../types';
import '../styles/ClientList.css';

const { Option } = Select;

/** 获取 OS 图标 */
function getOsIcon(os?: string): React.ReactNode {
  if (!os) return <DesktopOutlined />;
  const lower = os.toLowerCase();
  if (lower.includes('mac') || lower.includes('darwin')) return <AppleOutlined />;
  if (lower.includes('windows') || lower.includes('win')) return <WindowsOutlined />;
  return <DesktopOutlined />;
}

/** 格式化相对时间 */
function formatRelativeTime(dateStr: string): string {
  const date = new Date(dateStr);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMin = Math.floor(diffMs / 60000);
  const diffHour = Math.floor(diffMs / 3600000);
  const diffDay = Math.floor(diffMs / 86400000);

  if (diffMin < 1) return '刚刚';
  if (diffMin < 60) return `${diffMin} 分钟前`;
  if (diffHour < 24) return `${diffHour} 小时前`;
  if (diffDay < 7) return `${diffDay} 天前`;
  return date.toLocaleDateString('zh-CN', { year: 'numeric', month: '2-digit', day: '2-digit' });
}

export const ClientList: React.FC = () => {
  const [clients, setClients] = useState<Client[]>([]);
  const [stats, setStats] = useState<ClientStats | null>(null);
  const [loading, setLoading] = useState(false);

  // 筛选
  const [searchText, setSearchText] = useState('');
  const [onlineFilter, setOnlineFilter] = useState<string | undefined>(undefined);
  const [osFilter, setOsFilter] = useState<string | undefined>(undefined);

  const loadClients = useCallback(async () => {
    setLoading(true);
    try {
      const params: Record<string, any> = {};
      if (searchText) params.search = searchText;
      if (onlineFilter === 'online') params.online = true;
      if (onlineFilter === 'offline') params.online = false;
      if (osFilter) params.os = osFilter;

      const response = await apiClient.get('/clients', { params });
      setClients(response.data.clients || []);
    } catch (error: any) {
      message.error(error.response?.data?.error || '加载客户端列表失败');
    } finally {
      setLoading(false);
    }
  }, [searchText, onlineFilter, osFilter]);

  const loadStats = useCallback(async () => {
    try {
      const response = await apiClient.get('/clients/stats');
      setStats(response.data);
    } catch {
      // 统计加载失败不阻塞
    }
  }, []);

  useEffect(() => {
    loadClients();
  }, [loadClients]);

  useEffect(() => {
    loadStats();
  }, [loadStats]);

  const handleDelete = useCallback(async (id: string) => {
    try {
      await apiClient.delete(`/clients/${id}`);
      message.success('客户端记录已删除');
      loadClients();
      loadStats();
    } catch (error: any) {
      message.error(error.response?.data?.error || '删除失败');
    }
  }, [loadClients, loadStats]);

  const hasActiveFilters = searchText || onlineFilter || osFilter;

  const clearFilters = useCallback(() => {
    setSearchText('');
    setOnlineFilter(undefined);
    setOsFilter(undefined);
  }, []);

  // 从数据中提取 OS 选项
  const osOptions = useMemo(() => {
    const osSet = new Set(clients.map((c) => c.os).filter(Boolean));
    return Array.from(osSet) as string[];
  }, [clients]);

  const columns: ColumnsType<Client> = [
    {
      title: '客户端',
      key: 'client',
      width: 200,
      render: (_, record) => (
        <Space>
          {getOsIcon(record.os)}
          <span style={{ fontWeight: 500 }}>{record.client_name || record.username || record.id.slice(0, 8)}</span>
        </Space>
      ),
    },
    {
      title: '用户',
      dataIndex: 'username',
      key: 'username',
      width: 120,
      render: (name: string | undefined) => name || <span style={{ color: '#bfbfbf' }}>-</span>,
    },
    {
      title: '状态',
      dataIndex: 'online',
      key: 'online',
      width: 90,
      render: (online: boolean) => (
        <span className="client-online-badge">
          <span className={`client-online-dot ${online ? 'online' : 'offline'}`} />
          {online ? '在线' : '离线'}
        </span>
      ),
    },
    {
      title: '版本',
      dataIndex: 'version',
      key: 'version',
      width: 100,
      render: (v: string | undefined) => v ? <Tag>{v}</Tag> : <span style={{ color: '#bfbfbf' }}>-</span>,
    },
    {
      title: '操作系统',
      dataIndex: 'os',
      key: 'os',
      width: 130,
      render: (os: string | undefined) => os ? (
        <Space size={4}>
          {getOsIcon(os)}
          {os}
        </Space>
      ) : <span style={{ color: '#bfbfbf' }}>-</span>,
    },
    {
      title: 'IP 地址',
      dataIndex: 'ip_address',
      key: 'ip_address',
      width: 140,
      render: (ip: string | undefined) => ip ? <code>{ip}</code> : <span style={{ color: '#bfbfbf' }}>-</span>,
    },
    {
      title: '策略版本',
      dataIndex: 'policy_version',
      key: 'policy_version',
      width: 100,
      render: (v: string | undefined) => v ? <Tag color="blue">{v}</Tag> : <Tag>未同步</Tag>,
    },
    {
      title: '最后活跃',
      dataIndex: 'last_activity',
      key: 'last_activity',
      width: 140,
      sorter: (a, b) => new Date(a.last_activity).getTime() - new Date(b.last_activity).getTime(),
      render: (date: string) => (
        <Tooltip title={new Date(date).toLocaleString('zh-CN')}>
          {formatRelativeTime(date)}
        </Tooltip>
      ),
    },
    {
      title: '操作',
      key: 'action',
      width: 80,
      render: (_, record) => (
        <Popconfirm
          title="确认删除"
          description="确定要删除此客户端记录吗？"
          onConfirm={() => handleDelete(record.id)}
          okText="确定"
          cancelText="取消"
          okButtonProps={{ danger: true }}
        >
          <Button type="link" danger size="small" icon={<DeleteOutlined />}>
            删除
          </Button>
        </Popconfirm>
      ),
    },
  ];

  const emptyContent = clients.length === 0 && !loading ? (
    <Empty
      image={<CloudServerOutlined style={{ fontSize: 48, color: '#bfbfbf' }} />}
      description={
        hasActiveFilters ? (
          <span>
            没有匹配的客户端，<Button type="link" onClick={clearFilters} style={{ padding: 0 }}>清除筛选条件</Button>
          </span>
        ) : '暂无已注册的客户端，当桌面客户端连接后将自动显示'
      }
    />
  ) : undefined;

  return (
    <div className="client-list-container">
      <div className="page-header">
        <h2>客户端管理</h2>
        <Button icon={<ReloadOutlined />} onClick={() => { loadClients(); loadStats(); }} loading={loading}>
          刷新
        </Button>
      </div>

      {/* 统计卡片 */}
      {stats && (
        <Row gutter={16} className="client-stats-row">
          <Col span={6}>
            <Card size="small">
              <Statistic title="总客户端" value={stats.total} prefix={<DesktopOutlined />} />
            </Card>
          </Col>
          <Col span={6}>
            <Card size="small">
              <Statistic title="在线" value={stats.online} prefix={<CheckCircleOutlined />} valueStyle={{ color: '#52c41a' }} />
            </Card>
          </Col>
          <Col span={6}>
            <Card size="small">
              <Statistic title="离线" value={stats.offline} prefix={<CloseCircleOutlined />} valueStyle={{ color: '#999' }} />
            </Card>
          </Col>
          <Col span={6}>
            <Card size="small">
              <Statistic title="版本分布" value={stats.by_version.length} suffix="个版本" />
            </Card>
          </Col>
        </Row>
      )}

      {/* 筛选栏 */}
      <div className="client-filter-bar">
        <Input
          placeholder="搜索用户名、客户端名或 IP"
          prefix={<SearchOutlined />}
          value={searchText}
          onChange={(e) => setSearchText(e.target.value)}
          allowClear
          style={{ width: 260 }}
        />
        <Select
          placeholder="在线状态"
          value={onlineFilter}
          onChange={setOnlineFilter}
          allowClear
          style={{ width: 120 }}
        >
          <Option value="online">在线</Option>
          <Option value="offline">离线</Option>
        </Select>
        {osOptions.length > 0 && (
          <Select
            placeholder="操作系统"
            value={osFilter}
            onChange={setOsFilter}
            allowClear
            style={{ width: 140 }}
          >
            {osOptions.map((os) => (
              <Option key={os} value={os}>{os}</Option>
            ))}
          </Select>
        )}
        {hasActiveFilters && (
          <Button type="link" onClick={clearFilters} size="small">
            清除筛选
          </Button>
        )}
        <span className="client-filter-summary">
          共 {clients.length} 个客户端
        </span>
      </div>

      <Table
        columns={columns}
        dataSource={clients}
        rowKey="id"
        loading={loading}
        locale={{ emptyText: emptyContent }}
        pagination={clients.length > 10 ? {
          pageSize: 10,
          showSizeChanger: true,
          showTotal: (total) => `共 ${total} 个客户端`,
        } : false}
        scroll={{ x: 1100 }}
        size="middle"
      />
    </div>
  );
};
