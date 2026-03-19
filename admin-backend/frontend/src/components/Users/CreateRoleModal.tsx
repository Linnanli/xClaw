import React, { useState } from 'react';
import { Modal, Form, Input, message } from 'antd';
import { apiClient } from '../../api/client';

interface CreateRoleModalProps {
  visible: boolean;
  onCancel: () => void;
  onSuccess: () => void;
}

export const CreateRoleModal: React.FC<CreateRoleModalProps> = ({
  visible,
  onCancel,
  onSuccess,
}) => {
  const [form] = Form.useForm();
  const [loading, setLoading] = useState(false);

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();
      setLoading(true);

      await apiClient.post('/roles', {
        name: values.name,
        description: values.description,
      });

      message.success('创建角色成功');
      form.resetFields();
      onSuccess();
    } catch (error: any) {
      if (error.errorFields) {
        return;
      }
      message.error(error.response?.data?.error || '创建角色失败');
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
      title="创建角色"
      open={visible}
      onOk={handleSubmit}
      onCancel={handleCancel}
      confirmLoading={loading}
      okText="创建"
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
