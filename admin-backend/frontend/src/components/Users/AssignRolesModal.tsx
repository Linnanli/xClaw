import React, { useState, useEffect } from 'react';
import { Modal, Form, Select, message } from 'antd';
import { apiClient } from '../../api/client';
import type { Role } from '../../types';

interface AssignRolesModalProps {
  visible: boolean;
  userId: string;
  username: string;
  currentRoles: string[];
  onCancel: () => void;
  onSuccess: () => void;
}

export const AssignRolesModal: React.FC<AssignRolesModalProps> = ({
  visible,
  userId,
  username,
  currentRoles,
  onCancel,
  onSuccess,
}) => {
  const [form] = Form.useForm();
  const [loading, setLoading] = useState(false);
  const [roles, setRoles] = useState<Role[]>([]);
  const [loadingRoles, setLoadingRoles] = useState(false);

  // 加载角色列表
  const loadRoles = async () => {
    setLoadingRoles(true);
    try {
      const response = await apiClient.get('/roles');
      setRoles(response.data.roles || []);
    } catch (error: any) {
      message.error(error.response?.data?.error || '加载角色列表失败');
    } finally {
      setLoadingRoles(false);
    }
  };

  useEffect(() => {
    if (visible) {
      loadRoles();
      form.setFieldsValue({ role_ids: currentRoles });
    }
  }, [visible, currentRoles, form]);

  // 提交表单
  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();
      setLoading(true);

      await apiClient.post(`/users/${userId}/roles`, {
        role_ids: values.role_ids || [],
      });

      message.success('分配角色成功');
      form.resetFields();
      onSuccess();
    } catch (error: any) {
      if (error.response) {
        message.error(error.response.data?.error || '分配角色失败');
      }
    } finally {
      setLoading(false);
    }
  };

  return (
    <Modal
      title={`分配角色 - ${username}`}
      open={visible}
      onOk={handleSubmit}
      onCancel={onCancel}
      confirmLoading={loading}
      okText="保存"
      cancelText="取消"
      width={600}
    >
      <Form
        form={form}
        layout="vertical"
        initialValues={{ role_ids: currentRoles }}
      >
        <Form.Item
          label="角色"
          name="role_ids"
          rules={[
            { required: true, message: '请选择至少一个角色' },
          ]}
        >
          <Select
            mode="multiple"
            placeholder="请选择角色"
            loading={loadingRoles}
            options={roles.map((role) => ({
              label: `${role.name}${role.description ? ` - ${role.description}` : ''}`,
              value: role.id,
            }))}
            filterOption={(input, option) =>
              (option?.label ?? '').toLowerCase().includes(input.toLowerCase())
            }
          />
        </Form.Item>
      </Form>
    </Modal>
  );
};
