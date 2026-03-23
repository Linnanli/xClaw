import React, { useState, useEffect } from 'react';
import { Table, Button, Space, Popconfirm, Tag, message } from 'antd';
import { PlusOutlined, DeleteOutlined, ReloadOutlined, TeamOutlined, EditOutlined } from '@ant-design/icons';
import type { ColumnsType } from 'antd/es/table';
import { CreateUserModal } from '../../components/Users/CreateUserModal';
import { AssignRolesModal } from '../../components/Users/AssignRolesModal';
import { EditUserModal } from '../../components/Users/EditUserModal';
import { apiClient } from '../../api/client';
import type { User } from '../../types';
import '../../styles/UserList.css';

interface UserWithRoles extends User {
  roles?: Array<{ id: string; name: string }>;
  department?: { id: string; name: string } | null;
}

export const UserList: React.FC = () => {
  const [users, setUsers] = useState<UserWithRoles[]>([]);
  const [loading, setLoading] = useState(false);
  const [createModalVisible, setCreateModalVisible] = useState(false);
  const [assignRolesModalVisible, setAssignRolesModalVisible] = useState(false);
  const [editModalVisible, setEditModalVisible] = useState(false);
  const [selectedUser, setSelectedUser] = useState<UserWithRoles | null>(null);

  // 加载用户列表
  const loadUsers = async () => {
    setLoading(true);
    try {
      const response = await apiClient.get('/users');
      setUsers(response.data.users || []);
    } catch (error: any) {
      message.error(error.response?.data?.error || '加载用户列表失败');
    } finally {
      setLoading(false);
    }
  };

  // 打开编辑模态框
  const handleEdit = (user: UserWithRoles) => {
    setSelectedUser(user);
    setEditModalVisible(true);
  };

  // 编辑成功回调
  const handleEditSuccess = () => {
    setEditModalVisible(false);
    setSelectedUser(null);
    loadUsers();
  };

  // 打开分配角色模态框
  const handleAssignRoles = (user: UserWithRoles) => {
    setSelectedUser(user);
    setAssignRolesModalVisible(true);
  };

  // 分配角色成功回调
  const handleAssignRolesSuccess = () => {
    setAssignRolesModalVisible(false);
    setSelectedUser(null);
    loadUsers();
  };

  // 删除用户
  const handleDelete = async (userId: string) => {
    try {
      await apiClient.delete(`/users/${userId}`);
      message.success('删除用户成功');
      loadUsers();
    } catch (error: any) {
      message.error(error.response?.data?.error || '删除用户失败');
    }
  };

  // 创建用户成功回调
  const handleCreateSuccess = () => {
    setCreateModalVisible(false);
    loadUsers();
  };

  useEffect(() => {
    loadUsers();
  }, []);

  // 表格列配置
  const columns: ColumnsType<UserWithRoles> = [
    {
      title: '用户名',
      dataIndex: 'username',
      key: 'username',
      width: 150,
    },
    {
      title: '邮箱',
      dataIndex: 'email',
      key: 'email',
      width: 200,
    },
    {
      title: '角色',
      dataIndex: 'roles',
      key: 'roles',
      width: 200,
      render: (roles: Array<{ id: string; name: string }>) => (
        <>
          {roles && roles.length > 0 ? (
            roles.map((role) => (
              <Tag key={role.id} color="blue">
                {role.name}
              </Tag>
            ))
          ) : (
            <Tag color="default">未分配</Tag>
          )}
        </>
      ),
    },
    {
      title: '部门',
      key: 'department',
      width: 120,
      render: (_, record) => record.department ? (
        <Tag color="cyan">{record.department.name}</Tag>
      ) : (
        <Tag color="default">未分配</Tag>
      ),
    },
    {
      title: '创建时间',
      dataIndex: 'created_at',
      key: 'created_at',
      width: 180,
      render: (date: string) => new Date(date).toLocaleString('zh-CN'),
    },
    {
      title: '更新时间',
      dataIndex: 'updated_at',
      key: 'updated_at',
      width: 180,
      render: (date: string) => new Date(date).toLocaleString('zh-CN'),
    },
    {
      title: '操作',
      key: 'action',
      width: 200,
      fixed: 'right',
      render: (_, record) => (
        <Space size="small">
          <Button
            type="link"
            size="small"
            icon={<EditOutlined />}
            onClick={() => handleEdit(record)}
          >
            编辑
          </Button>
          <Button
            type="link"
            size="small"
            icon={<TeamOutlined />}
            onClick={() => handleAssignRoles(record)}
          >
            分配角色
          </Button>
          <Popconfirm
            title="确认删除"
            description={`确定要删除用户 "${record.username}" 吗？`}
            onConfirm={() => handleDelete(record.id)}
            okText="确定"
            cancelText="取消"
          >
            <Button
              type="link"
              danger
              size="small"
              icon={<DeleteOutlined />}
            >
              删除
            </Button>
          </Popconfirm>
        </Space>
      ),
    },
  ];

  return (
    <div className="user-list-container">
      <div className="page-header">
        <h2>用户管理</h2>
        <Space>
          <Button
            icon={<ReloadOutlined />}
            onClick={loadUsers}
            loading={loading}
          >
            刷新
          </Button>
          <Button
            type="primary"
            icon={<PlusOutlined />}
            onClick={() => setCreateModalVisible(true)}
          >
            创建用户
          </Button>
        </Space>
      </div>

      <Table
        columns={columns}
        dataSource={users}
        rowKey="id"
        loading={loading}
        pagination={{
          pageSize: 10,
          showSizeChanger: true,
          showTotal: (total) => `共 ${total} 个用户`,
        }}
        scroll={{ x: 1000 }}
      />

      <CreateUserModal
        visible={createModalVisible}
        onCancel={() => setCreateModalVisible(false)}
        onSuccess={handleCreateSuccess}
      />

      {selectedUser && (
        <AssignRolesModal
          visible={assignRolesModalVisible}
          userId={selectedUser.id}
          username={selectedUser.username}
          currentRoles={selectedUser.roles?.map((r) => r.id) || []}
          onCancel={() => {
            setAssignRolesModalVisible(false);
            setSelectedUser(null);
          }}
          onSuccess={handleAssignRolesSuccess}
        />
      )}

      <EditUserModal
        visible={editModalVisible}
        user={selectedUser}
        onCancel={() => {
          setEditModalVisible(false);
          setSelectedUser(null);
        }}
        onSuccess={handleEditSuccess}
      />
    </div>
  );
};
