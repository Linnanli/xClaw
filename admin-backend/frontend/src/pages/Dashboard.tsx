import React, { useEffect, useState, useCallback } from 'react';
import { Card, Row, Col, Statistic, Table, Tag, message } from 'antd';
import {
  UserOutlined,
  TeamOutlined,
  SafetyOutlined,
  WarningOutlined,
  ReloadOutlined,
} from '@ant-design/icons';
import { Line } from '@ant-design/charts';
import type { ColumnsType } from 'antd/es/table';
import { apiClient } from '../api/client';
import '../styles/Dashboard.css';

interface DashboardStats {
  total_users: number;
  online_clients: number;
  dlp_blocked_today: number;
  sensitive_ops_today: number;
}

interface ActivityLog {
  id: string;
  username: string | null;
  action: string;
  details: string;
  created_at: string;
}

interface TrendPoint {
  date: string;
  count: number;
}

interface TrendsData {
  user_activity: TrendPoint[];
  dlp_blocks: TrendPoint[];
}

export const Dashboard: React.FC = () => {
  const [stats, setStats] = useState<DashboardStats>({
    total_users: 0,
    online_clients: 0,
    dlp_blocked_today: 0,
    sensitive_ops_today: 0,
  });
  const [activityLogs, setActivityLogs] = useState<ActivityLog[]>([]);
  const [trends, setTrends] = useState<TrendsData>({ user_activity: [], dlp_blocks: [] });
  const [loading, setLoading] = useState(true);

  const loadDashboard = useCallback(async () => {
    setLoading(true);
    try {
      const [statsRes, activityRes, trendsRes] = await Promise.allSettled([
        apiClient.get('/dashboard/stats'),
        apiClient.get('/dashboard/activity'),
        apiClient.get('/dashboard/trends'),
      ]);

      if (statsRes.status === 'fulfilled') {
        setStats(statsRes.value.data);
      }
      if (activityRes.status === 'fulfilled') {
        setActivityLogs(activityRes.value.data.logs || []);
      }
      if (trendsRes.status === 'fulfilled') {
        setTrends({
          user_activity: trendsRes.value.data.user_activity || [],
          dlp_blocks: trendsRes.value.data.dlp_blocks || [],
        });
      }
    } catch (error: any) {
      message.error('加载仪表盘数据失败');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadDashboard();
  }, [loadDashboard]);

  const columns: ColumnsType<ActivityLog> = [
    {
      title: '用户',
      dataIndex: 'username',
      key: 'username',
      render: (name: string | null) => name || <Tag color="default">系统</Tag>,
    },
    {
      title: '操作',
      dataIndex: 'action',
      key: 'action',
      render: (action: string) => <Tag color="blue">{action}</Tag>,
    },
    {
      title: '详情',
      dataIndex: 'details',
      key: 'details',
      ellipsis: true,
    },
    {
      title: '时间',
      dataIndex: 'created_at',
      key: 'created_at',
      render: (date: string) => new Date(date).toLocaleString('zh-CN'),
    },
  ];

  const chartConfig = {
    xField: 'date',
    yField: 'count',
    smooth: true,
    point: { size: 5, shape: 'circle' as const },
    animation: { appear: { animation: 'path-in' as const, duration: 1000 } },
  };

  return (
    <div className="dashboard-container">
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
        <h2 style={{ margin: 0 }}>仪表盘</h2>
        <span style={{ cursor: 'pointer', color: '#1890ff' }} onClick={loadDashboard}>
          <ReloadOutlined spin={loading} /> 刷新
        </span>
      </div>

      <Row gutter={[16, 16]} className="stats-row">
        <Col xs={24} sm={12} lg={6}>
          <Card><Statistic title="总用户数" value={stats.total_users} prefix={<UserOutlined />} loading={loading} /></Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card><Statistic title="在线客户端" value={stats.online_clients} prefix={<TeamOutlined />} valueStyle={{ color: '#3f8600' }} loading={loading} /></Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card><Statistic title="DLP 拦截（今日）" value={stats.dlp_blocked_today} prefix={<SafetyOutlined />} valueStyle={{ color: '#cf1322' }} loading={loading} /></Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card><Statistic title="敏感操作（今日）" value={stats.sensitive_ops_today} prefix={<WarningOutlined />} valueStyle={{ color: '#faad14' }} loading={loading} /></Card>
        </Col>
      </Row>

      <Row gutter={[16, 16]} className="charts-row">
        <Col xs={24} lg={12}>
          <Card title="用户活动趋势（近7天）" loading={loading}>
            <Line {...chartConfig} data={trends.user_activity} height={300} />
          </Card>
        </Col>
        <Col xs={24} lg={12}>
          <Card title="DLP 拦截趋势（近7天）" loading={loading}>
            <Line {...chartConfig} data={trends.dlp_blocks} height={300} />
          </Card>
        </Col>
      </Row>

      <Card title="最近操作日志" className="activity-logs-card">
        <Table columns={columns} dataSource={activityLogs} rowKey="id" loading={loading} pagination={false} size="middle" />
      </Card>
    </div>
  );
};
