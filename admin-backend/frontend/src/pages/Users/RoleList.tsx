import React, { useState, useEffect } from 'react';
import { Table, Button, Space, Modal, message, Popconfirm, Tag } from 'antd';
import { PlusOutlined, DeleteOutlined, ReloadOutlined, EditOutlined, KeyOutlined } from '@ant-design/icons';
import type { ColumnsType } from 'antd/es/table';
import { CreateRoleModal } from '../../components/Users/CreateRoleModal';
import { EditRoleModal } from '../../components/Users/EditRoleModal';
import { AssignPermissionsModal } from '../../components/Users/AssignPermissionsModal';
import { apiClient } from '../../api/client';
import type { Role } from '../../types';
import '../../styles/RoleList.css';

export const RoleList: React.FC = () => {
  const [roles, setRoles] = useState<Role[]>([]);
  const [loading, setLoading] = useState(false);
  const [createModalVisible, setCreateModalVisible] = useState(false);
  const [editModalVisible, setEditModalVisible] = useState(false);
  const [assignModalVisible, setAssignModalVisible] = useState(false);
  const [selectedRole, setSelectedRole] = useState<Role | null>(null);

  const loadRoles = async () => {
    setLoading(true);
    try {
      const response = await apiClient.get('/roles');
      setRoles(response.data.roles || []);
    } catch (error: any) {
      message.error(error.response?.data?.error || '加载角色列表失败');
    } finally {
      setLoading(false);
    }
  };

  const handleDelete = async (roleId: string) => {
    try {
      await apiClient.delete(`/roles/${roleId}`);
      message.success('删除角色成功');
      loadRoles();
    } catch (error: any) {
      message.error(error.response?.data?.error || '删除角色失败');
    }
  };

  const handleEdit = (role: Role) => {
    setSelectedRole(role);
    setEditModalVisible(true);
  };

  const handleAssignPermissions = (role: Role) => {
    setSelectedRole(role);
    setAssignModalVisible(true);
  };

  const handleCreateSuccess = () => {
    setCreateModalVisible(false);
    loadRoles();
  };

  const handleEditSuccess = () => {
    setEditModalVisible(false);
    setSelectedRole(null);
    loadRoles();
  };

  const handleAssignSuccess = () => {
    setAssignModalVisible(false);
    setSelectedRole(null);
    loadRoles();
  };

  useEffect(() => {
    loadRoles();
  }, []);

  const columns: ColumnsType<Role> = [
    {
      title: '角色名',
      dataIndex: 'name',
      key: 'name',
      width: 150,
      render: (name: string) => <Tag color="blue">{name}</Tag>,
    },
    {
      title: '描述',
      dataIndex: 'description',
      key: 'description',
      width: 250,
    },
    {
      title: '权限数量',
      dataIndex: 'permission_count',
      key: 'permission_count',
      width: 120,
      render: (count: number) => <Tag color="green">{count} 个权限</Tag>,
    },
    {
      title: '用户数量',
      dataIndex: 'user_count',
      key: 'user_count',
      width: 120,
      render: (count: number) => <Tag color="orange">{count} 个用户</Tag>,
    },
    {
      title: '创建时间',
      dataIndex: 'created_at',
      key: 'created_at',
      width: 180,
      render: (date: string) => new Date(date).toLocaleString('zh-CN'),
    },
    {
      title: '操作',
      key: 'action',
      width: 250,
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
            icon={<KeyOutlined />}
            onClick={() => handleAssignPermissions(record)}
          >
            分配权限
          </Button>
          <Popconfirm
            title="确认删除"
            description={`确定要删除角色 "${record.name}" 吗？`}
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
    <div className="role-list-container">
      <div className="page-header">
        <h2>角色管理</h2>
        <Space>
          <Button
            icon={<ReloadOutlined />}
            onClick={loadRoles}
            loading={loading}
          >
            刷新
          </Button>
          <Button
            type="primary"
            icon={<PlusOutlined />}
            onClick={() => setCreateModalVisible(true)}
          >
            创建角色
          </Button>
        </Space>
      </div>

      <Table
        columns={columns}
        dataSource={roles}
        rowKey="id"
        loading={loading}
        pagination={{
          pageSize: 10,
          showSizeChanger: true,
          showTotal: (total) => `共 ${total} 个角色`,
        }}
        scroll={{ x: 1200 }}
      />

      <CreateRoleModal
        visible={createModalVisible}
        onCancel={() => setCreateModalVisible(false)}
        onSuccess={handleCreateSuccess}
      />

      {selectedRole && (
        <>
          <EditRoleModal
            visible={editModalVisible}
            role={selectedRole}
            onCancel={() => {
              setEditModalVisible(false);
              setSelectedRole(null);
            }}
            onSuccess={handleEditSuccess}
          />

          <AssignPermissionsModal
            visible={assignModalVisible}
            role={selectedRole}
            onCancel={() => {
              setAssignModalVisible(false);
              setSelectedRole(null);
            }}
            onSuccess={handleAssignSuccess}
          />
        </>
      )}
    </div>
  );
};
