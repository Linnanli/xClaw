import React, { useEffect, useState } from 'react';
import { Modal, Form, Input, Select, message } from 'antd';
import { apiClient } from '../../api/client';
import type { Department } from '../../types';

interface EditUserModalProps {
  visible: boolean;
  user: { id: string; username: string; email: string; department?: { id: string; name: string } | null } | null;
  onCancel: () => void;
  onSuccess: () => void;
}

export const EditUserModal: React.FC<EditUserModalProps> = ({ visible, user, onCancel, onSuccess }) => {
  const [form] = Form.useForm();
  const [loading, setLoading] = useState(false);
  const [departments, setDepartments] = useState<Department[]>([]);

  useEffect(() => {
    if (visible && user) {
      form.setFieldsValue({
        email: user.email,
        department_id: user.department?.id || undefined,
        password: undefined,
      });
      loadDepartments();
    }
  }, [visible, user, form]);

  const loadDepartments = async () => {
    try {
      const res = await apiClient.get('/departments');
      setDepartments(res.data.departments || []);
    } catch { /* ignore */ }
  };

  const handleSubmit = async () => {
    if (!user) return;
    try {
      const values = await form.validateFields();
      setLoading(true);

      const payload: Record<string, any> = {};
      if (values.email && values.email !== user.email) payload.email = values.email;
      if (values.password) payload.password = values.password;
      // department_id: undefined=不修改, null=清除, string=设置
      if (values.department_id !== undefined) {
        payload.department_id = values.department_id || null;
      }

      if (Object.keys(payload).length === 0) {
        message.info('没有需要更新的字段');
        return;
      }

      await apiClient.put(`/users/${user.id}`, payload);
      message.success('用户信息已更新');
      onSuccess();
    } catch (error: any) {
      message.error(error.response?.data?.details || error.response?.data?.error || '更新失败');
    } finally {
      setLoading(false);
    }
  };

  return (
    <Modal
      title={`编辑用户: ${user?.username || ''}`}
      open={visible}
      onOk={handleSubmit}
      onCancel={onCancel}
      confirmLoading={loading}
      destroyOnClose
    >
      <Form form={form} layout="vertical">
        <Form.Item label="邮箱" name="email" rules={[{ type: 'email', message: '请输入有效的邮箱地址' }]}>
          <Input placeholder="输入新邮箱" />
        </Form.Item>
        <Form.Item label="密码" name="password" rules={[{ min: 8, message: '密码至少 8 个字符' }]}>
          <Input.Password placeholder="留空则不修改密码" />
        </Form.Item>
        <Form.Item label="部门" name="department_id">
          <Select placeholder="选择部门" allowClear>
            {departments.map((d) => (
              <Select.Option key={d.id} value={d.id}>{d.name}</Select.Option>
            ))}
          </Select>
        </Form.Item>
      </Form>
    </Modal>
  );
};
