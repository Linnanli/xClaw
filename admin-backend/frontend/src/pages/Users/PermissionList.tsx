import React, { useState, useEffect } from 'react';
import { Card, Table, Tag, message, Spin } from 'antd';
import type { ColumnsType } from 'antd/es/table';
import { apiClient } from '../../api/client';
import type { Permission } from '../../types';
import '../../styles/PermissionList.css';

export const PermissionList: React.FC = () => {
  const [loading, setLoading] = useState(false);
  const [permissions, setPermissions] = useState<Record<string, Permission[]>>({});

  const loadPermissions = async () => {
    setLoading(true);
    try {
      const response = await apiClient.get('/permissions');
      setPermissions(response.data.permissions || {});
    } catch (error: any) {
      message.error(error.response?.data?.error || '加载权限列表失败');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadPermissions();
  }, []);

  const columns: ColumnsType<Permission> = [
    {
      title: '权限名',
      dataIndex: 'name',
      key: 'name',
      width: 200,
      render: (name: string) => <Tag color="blue">{name}</Tag>,
    },
    {
      title: '描述',
      dataIndex: 'description',
      key: 'description',
      width: 300,
    },
    {
      title: '资源',
      dataIndex: 'resource',
      key: 'resource',
      width: 150,
      render: (resource: string) => <Tag color="green">{resource}</Tag>,
    },
    {
      title: '操作',
      dataIndex: 'action',
      key: 'action',
      width: 150,
      render: (action: string) => <Tag color="orange">{action}</Tag>,
    },
    {
      title: '创建时间',
      dataIndex: 'created_at',
      key: 'created_at',
      width: 180,
      render: (date: string) => new Date(date).toLocaleString('zh-CN'),
    },
  ];

  const resourceNames: Record<string, string> = {
    users: '用户管理',
    roles: '角色管理',
    permissions: '权限管理',
    dlp: 'DLP 管理',
    audit: '审计日志',
  };

  return (
    <div className="permission-list-container">
      <div className="page-header">
        <h2>权限管理</h2>
      </div>

      {loading ? (
        <div style={{ textAlign: 'center', padding: '100px 0' }}>
          <Spin size="large" tip="加载中..." />
        </div>
      ) : (
        <div className="permission-groups">
          {Object.entries(permissions).map(([resource, perms]) => (
            <Card
              key={resource}
              title={resourceNames[resource] || resource}
              style={{ marginBottom: '24px' }}
            >
              <Table
                columns={columns}
                dataSource={perms}
                rowKey="id"
                pagination={false}
                size="small"
              />
            </Card>
          ))}
        </div>
      )}
    </div>
  );
};
