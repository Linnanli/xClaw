import React, { useState, useEffect } from 'react';
import { Modal, Form, Input, message } from 'antd';
import { apiClient } from '../../api/client';
import type { Role } from '../../types';

interface EditRoleModalProps {
  visible: boolean;
  role: Role;
  onCancel: () => void;
  onSuccess: () => void;
}

export const EditRoleModal: React.FC<EditRoleModalProps> = ({
  visible,
  role,
  onCancel,
  onSuccess,
}) => {
  const [form] = Form.useForm();
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (visible && role) {
      form.setFieldsValue({
        name: role.name,
        description: role.description,
      });
    }
  }, [visible, role, form]);

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();
      setLoading(true);

      await apiClient.put(`/roles/${role.id}`, {
        name: values.name,
        description: values.description,
      });

      message.success('更新角色成功');
      onSuccess();
    } catch (error: any) {
      if (error.errorFields) {
        return;
      }
      message.error(error.response?.data?.error || '更新角色失败');
    } finally {
      setLoading(false);
    }
  };

  const handleCancel = () => {
    form.resetFields();
    onCancel();
  };

  return (
    <Modal
      title="编辑角色"
      open={visible}
      onOk={handleSubmit}
      onCancel={handleCancel}
      confirmLoading={loading}
      okText="保存"
      cancelText="取消"
      destroyOnClose
    >
      <Form
        form={form}
        layout="vertical"
        autoComplete="off"
      >
        <Form.Item
          name="name"
          label="角色名"
          rules={[
            { required: true, message: '请输入角色名' },
            { min: 2, message: '角色名至少2个字符' },
            { max: 50, message: '角色名最多50个字符' },
            { pattern: /^[a-zA-Z0-9_-]+$/, message: '角色名只能包含字母、数字、下划线和连字符' },
          ]}
        >
          <Input placeholder="请输入角色名" />
        </Form.Item>

        <Form.Item
          name="description"
          label="描述"
          rules={[
            { max: 200, message: '描述最多200个字符' },
          ]}
        >
          <Input.TextArea
            placeholder="请输入角色描述"
            rows={4}
          />
        </Form.Item>
      </Form>
    </Modal>
  );
};
