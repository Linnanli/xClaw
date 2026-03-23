import React, { useState, useEffect, useCallback } from 'react';
import { Table, Button, Space, message, Popconfirm, Tag, Modal, Form, Input, InputNumber, Switch } from 'antd';
import { PlusOutlined, DeleteOutlined, ReloadOutlined, EditOutlined, TeamOutlined } from '@ant-design/icons';
import type { ColumnsType } from 'antd/es/table';
import { apiClient } from '../../api/client';
import type { Department } from '../../types';

export const DepartmentList: React.FC = () => {
  const [departments, setDepartments] = useState<Department[]>([]);
  const [loading, setLoading] = useState(false);
  const [modalVisible, setModalVisible] = useState(false);
  const [editingDept, setEditingDept] = useState<Department | null>(null);
  const [form] = Form.useForm();
  const [submitting, setSubmitting] = useState(false);

  const loadDepartments = useCallback(async () => {
    setLoading(true);
    try {
      const res = await apiClient.get('/departments');
      setDepartments(res.data.departments || []);
    } catch (error: any) {
      message.error(error.response?.data?.error || '加载部门列表失败');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { loadDepartments(); }, [loadDepartments]);

  const openCreate = () => {
    setEditingDept(null);
    form.resetFields();
    form.setFieldsValue({ token_quota_enabled: false });
    setModalVisible(true);
  };

  const openEdit = (dept: Department) => {
    setEditingDept(dept);
    form.setFieldsValue({
      name: dept.name,
      description: dept.description,
      token_quota_enabled: dept.token_quota_enabled,
      token_quota_per_day: dept.token_quota_per_day,
    });
    setModalVisible(true);
  };

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();
      setSubmitting(true);
      const payload = {
        name: values.name,
        description: values.description || null,
        token_quota_enabled: values.token_quota_enabled || false,
        token_quota_per_day: values.token_quota_enabled ? values.token_quota_per_day : null,
      };

      if (editingDept) {
        await apiClient.put(`/departments/${editingDept.id}`, payload);
        message.success('部门已更新');
      } else {
        await apiClient.post('/departments', payload);
        message.success('部门已创建');
      }
      setModalVisible(false);
      loadDepartments();
    } catch (error: any) {
      if (error.response) {
        message.error(error.response.data?.details || error.response.data?.error || '操作失败');
      }
    } finally {
      setSubmitting(false);
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await apiClient.delete(`/departments/${id}`);
      message.success('部门已删除');
      loadDepartments();
    } catch (error: any) {
      message.error(error.response?.data?.details || error.response?.data?.error || '删除失败');
    }
  };

  const quotaEnabled = Form.useWatch('token_quota_enabled', form);

  const columns: ColumnsType<Department> = [
    { title: '部门名称', dataIndex: 'name', key: 'name', width: 180, render: (name: string) => <span style={{ fontWeight: 500 }}>{name}</span> },
    { title: '描述', dataIndex: 'description', key: 'description', ellipsis: true, render: (d: string | undefined) => d || <span style={{ color: '#bfbfbf' }}>-</span> },
    { title: '成员数', dataIndex: 'member_count', key: 'member_count', width: 100, render: (c: number) => <Tag icon={<TeamOutlined />} color="blue">{c}</Tag> },
    {
      title: 'Token 限额',
      key: 'quota',
      width: 160,
      render: (_, record) => record.token_quota_enabled ? (
        <Tag color="orange">{record.token_quota_per_day?.toLocaleString() || 0} / 天</Tag>
      ) : (
        <Tag color="default">未启用</Tag>
      ),
    },
    { title: '创建时间', dataIndex: 'created_at', key: 'created_at', width: 170, render: (d: string) => new Date(d).toLocaleString('zh-CN') },
    {
      title: '操作', key: 'action', width: 160,
      render: (_, record) => (
        <Space size="small">
          <Button type="link" size="small" icon={<EditOutlined />} onClick={() => openEdit(record)}>编辑</Button>
          <Popconfirm title="确认删除" description={record.member_count > 0 ? '该部门下有用户，无法删除' : `确定要删除部门 "${record.name}" 吗？`} onConfirm={() => handleDelete(record.id)} okText="确定" cancelText="取消" okButtonProps={{ disabled: record.member_count > 0, danger: true }}>
            <Button type="link" danger size="small" icon={<DeleteOutlined />} disabled={record.member_count > 0}>删除</Button>
          </Popconfirm>
        </Space>
      ),
    },
  ];

  return (
    <div>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
        <h2 style={{ margin: 0 }}>部门管理</h2>
        <Space>
          <Button icon={<ReloadOutlined />} onClick={loadDepartments} loading={loading}>刷新</Button>
          <Button type="primary" icon={<PlusOutlined />} onClick={openCreate}>创建部门</Button>
        </Space>
      </div>

      <Table columns={columns} dataSource={departments} rowKey="id" loading={loading} pagination={departments.length > 10 ? { pageSize: 10, showTotal: (t) => `共 ${t} 个部门` } : false} />

      <Modal title={editingDept ? '编辑部门' : '创建部门'} open={modalVisible} onOk={handleSubmit} onCancel={() => setModalVisible(false)} confirmLoading={submitting} destroyOnClose>
        <Form form={form} layout="vertical">
          <Form.Item label="部门名称" name="name" rules={[{ required: true, message: '请输入部门名称' }, { min: 2, max: 100, message: '名称长度 2-100 字符' }]}>
            <Input placeholder="输入部门名称" />
          </Form.Item>
          <Form.Item label="描述" name="description">
            <Input.TextArea placeholder="输入部门描述（可选）" rows={2} />
          </Form.Item>
          <Form.Item label="启用 Token 限额" name="token_quota_enabled" valuePropName="checked">
            <Switch />
          </Form.Item>
          {quotaEnabled && (
            <Form.Item label="每日 Token 限额" name="token_quota_per_day" rules={[{ required: true, message: '请输入限额' }]}>
              <InputNumber min={0} style={{ width: '100%' }} placeholder="输入每日 Token 限额" />
            </Form.Item>
          )}
        </Form>
      </Modal>
    </div>
  );
};
