import React, { useState } from 'react';
import { Modal, Form, Input, Select, message } from 'antd';
import { apiClient } from '../../api/client';
import type { CreateDlpRuleRequest } from '../../types';
import { DLP_SEVERITY_OPTIONS, DLP_CATEGORY_OPTIONS } from '../../constants/dlp';

const { TextArea } = Input;
const { Option } = Select;

interface CreateDlpRuleModalProps {
  visible: boolean;
  onCancel: () => void;
  onSuccess: () => void;
}

export const CreateDlpRuleModal: React.FC<CreateDlpRuleModalProps> = ({
  visible,
  onCancel,
  onSuccess,
}) => {
  const [form] = Form.useForm();
  const [loading, setLoading] = useState(false);

  // 提交表单
  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();
      setLoading(true);

      const request: CreateDlpRuleRequest = {
        name: values.name,
        pattern: values.pattern,
        replacement: values.replacement,
        severity: values.severity,
        description: values.description,
        category: values.category,
      };

      await apiClient.post('/dlp-rules', request);

      message.success('创建规则成功');
      form.resetFields();
      onSuccess();
    } catch (error: any) {
      if (error.response) {
        message.error(error.response.data?.error || '创建规则失败');
      }
    } finally {
      setLoading(false);
    }
  };

  return (
    <Modal
      title="创建 DLP 规则"
      open={visible}
      onOk={handleSubmit}
      onCancel={onCancel}
      confirmLoading={loading}
      okText="创建"
      cancelText="取消"
      width={600}
    >
      <Form
        form={form}
        layout="vertical"
        initialValues={{
          severity: 'medium',
          category: 'pii',
        }}
      >
        <Form.Item
          label="规则名"
          name="name"
          rules={[
            { required: true, message: '请输入规则名' },
            { min: 2, max: 50, message: '规则名长度为 2-50 个字符' },
          ]}
        >
          <Input placeholder="例如：身份证号" />
        </Form.Item>

        <Form.Item
          label="匹配模式"
          name="pattern"
          rules={[
            { required: true, message: '请输入匹配模式' },
            { min: 1, max: 500, message: '匹配模式长度为 1-500 个字符' },
          ]}
        >
          <Input placeholder="例如：正则表达式" />
        </Form.Item>

        <Form.Item
          label="替换规则"
          name="replacement"
          rules={[
            { max: 100, message: '替换规则长度不超过 100 个字符' },
          ]}
        >
          <Input placeholder="例如：***（可选）" />
        </Form.Item>

        <Form.Item
          label="严重级别"
          name="severity"
          rules={[{ required: true, message: '请选择严重级别' }]}
        >
          <Select placeholder="请选择严重级别">
            {DLP_SEVERITY_OPTIONS.map(option => (
              <Option key={option.value} value={option.value}>
                {option.label}
              </Option>
            ))}
          </Select>
        </Form.Item>

        <Form.Item
          label="分类"
          name="category"
          rules={[{ required: true, message: '请选择分类' }]}
        >
          <Select placeholder="请选择分类">
            {DLP_CATEGORY_OPTIONS.map(option => (
              <Option key={option.value} value={option.value}>
                {option.label}
              </Option>
            ))}
          </Select>
        </Form.Item>

        <Form.Item
          label="描述"
          name="description"
          rules={[
            { max: 200, message: '描述长度不超过 200 个字符' },
          ]}
        >
          <TextArea
            rows={3}
            placeholder="规则描述（可选）"
          />
        </Form.Item>
      </Form>
    </Modal>
  );
};
