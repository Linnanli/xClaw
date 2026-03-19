import React, { useEffect, useState } from 'react';
import { Card, Row, Col, Statistic, Table, Tag } from 'antd';
import {
  UserOutlined,
  TeamOutlined,
  SafetyOutlined,
  WarningOutlined,
} from '@ant-design/icons';
import { Line } from '@ant-design/charts';
import type { ColumnsType } from 'antd/es/table';
import '../styles/Dashboard.css';

interface DashboardStats {
  totalUsers: number;
  onlineUsers: number;
  dlpBlocked: number;
  sensitiveOps: number;
}

interface ActivityLog {
  id: string;
  username: string;
  action: string;
  timestamp: string;
  status: 'success' | 'warning' | 'error';
}

export const Dashboard: React.FC = () => {
  const [stats, setStats] = useState<DashboardStats>({
    totalUsers: 0,
    onlineUsers: 0,
    dlpBlocked: 0,
    sensitiveOps: 0,
  });

  const [activityLogs, setActivityLogs] = useState<ActivityLog[]>([]);
  const [loading, setLoading] = useState(true);

  // 模拟数据加载
  useEffect(() => {
    // TODO: 替换为真实 API 调用
    setTimeout(() => {
      setStats({
        totalUsers: 156,
        onlineUsers: 42,
        dlpBlocked: 23,
        sensitiveOps: 8,
      });

      setActivityLogs([
        {
          id: '1',
          username: 'admin',
          action: '创建用户',
          timestamp: '2026-03-19 14:30:00',
          status: 'success',
        },
        {
          id: '2',
          username: 'user1',
          action: 'DLP 拦截',
          timestamp: '2026-03-19 14:25:00',
          status: 'warning',
        },
        {
          id: '3',
          username: 'user2',
          action: '登录失败',
          timestamp: '2026-03-19 14:20:00',
          status: 'error',
        },
        {
          id: '4',
          username: 'admin',
          action: '更新 DLP 规则',
          timestamp: '2026-03-19 14:15:00',
          status: 'success',
        },
        {
          id: '5',
          username: 'user3',
          action: '敏感操作审批',
          timestamp: '2026-03-19 14:10:00',
          status: 'warning',
        },
      ]);

      setLoading(false);
    }, 1000);
  }, []);

  // 用户活动趋势数据
  const userActivityData = [
    { date: '03-13', count: 120 },
    { date: '03-14', count: 132 },
    { date: '03-15', count: 145 },
    { date: '03-16', count: 138 },
    { date: '03-17', count: 156 },
    { date: '03-18', count: 162 },
    { date: '03-19', count: 178 },
  ];

  // DLP 拦截趋势数据
  const dlpBlockData = [
    { date: '03-13', count: 15 },
    { date: '03-14', count: 18 },
    { date: '03-15', count: 22 },
    { date: '03-16', count: 19 },
    { date: '03-17', count: 25 },
    { date: '03-18', count: 21 },
    { date: '03-19', count: 23 },
  ];

  // 活动日志表格列配置
  const columns: ColumnsType<ActivityLog> = [
    {
      title: '用户',
      dataIndex: 'username',
      key: 'username',
    },
    {
      title: '操作',
      dataIndex: 'action',
      key: 'action',
    },
    {
      title: '时间',
      dataIndex: 'timestamp',
      key: 'timestamp',
    },
    {
      title: '状态',
      dataIndex: 'status',
      key: 'status',
      render: (status: string) => {
        const colorMap: Record<string, string> = {
          success: 'green',
          warning: 'orange',
          error: 'red',
        };
        const textMap: Record<string, string> = {
          success: '成功',
          warning: '警告',
          error: '失败',
        };
        return <Tag color={colorMap[status]}>{textMap[status]}</Tag>;
      },
    },
  ];

  // 图表配置
  const chartConfig = {
    xField: 'date',
    yField: 'count',
    smooth: true,
    point: {
      size: 5,
      shape: 'circle',
    },
    label: {
      style: {
        fill: '#aaa',
      },
    },
    animation: {
      appear: {
        animation: 'path-in',
        duration: 1000,
      },
    },
  };

  return (
    <div className="dashboard-container">
      <h2>仪表盘</h2>

      {/* 统计卡片 */}
      <Row gutter={[16, 16]} className="stats-row">
        <Col xs={24} sm={12} lg={6}>
          <Card>
            <Statistic
              title="总用户数"
              value={stats.totalUsers}
              prefix={<UserOutlined />}
              loading={loading}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card>
            <Statistic
              title="在线用户"
              value={stats.onlineUsers}
              prefix={<TeamOutlined />}
              valueStyle={{ color: '#3f8600' }}
              loading={loading}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card>
            <Statistic
              title="DLP 拦截（今日）"
              value={stats.dlpBlocked}
              prefix={<SafetyOutlined />}
              valueStyle={{ color: '#cf1322' }}
              loading={loading}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card>
            <Statistic
              title="敏感操作（今日）"
              value={stats.sensitiveOps}
              prefix={<WarningOutlined />}
              valueStyle={{ color: '#faad14' }}
              loading={loading}
            />
          </Card>
        </Col>
      </Row>

      {/* 趋势图表 */}
      <Row gutter={[16, 16]} className="charts-row">
        <Col xs={24} lg={12}>
          <Card title="用户活动趋势" loading={loading}>
            <Line {...chartConfig} data={userActivityData} height={300} />
          </Card>
        </Col>
        <Col xs={24} lg={12}>
          <Card title="DLP 拦截趋势" loading={loading}>
            <Line {...chartConfig} data={dlpBlockData} height={300} />
          </Card>
        </Col>
      </Row>

      {/* 最近操作日志 */}
      <Card title="最近操作日志" className="activity-logs-card">
        <Table
          columns={columns}
          dataSource={activityLogs}
          rowKey="id"
          loading={loading}
          pagination={false}
        />
      </Card>
    </div>
  );
};
