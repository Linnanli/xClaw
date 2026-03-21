import React, { useState, useEffect } from 'react';
import { Modal, Form, Input, Select, Switch, Space, Tag, message } from 'antd';
import { apiClient } from '../../api/client';
import type { SensitiveOperation, CreateSensitiveOperationRequest } from '../../types';
import {
  SENSITIVE_OP_TYPE_OPTIONS,
  RISK_LEVEL_OPTIONS,
} from '../../constants/sensitiveOps';

const { Option } = Select;
const { TextArea } = Input;

export type SensitiveOpFormMode = 'create' | 'edit';

export interface SensitiveOpFormModalProps {
  visible: boolean;
  mode: SensitiveOpFormMode;
  operation?: SensitiveOperation | null;
  onCancel: () => void;
  onSuccess: () => void;
}

export const SensitiveOpFormModal: React.FC<SensitiveOpFormModalProps> = ({
  visible,
  mode,
  operation,
  onCancel,
  onSuccess,
}) => {
  const [form] = Form.useForm();
  const [submitting, setSubmitting] = useState(false);
  const [requiresApproval, setRequiresApproval] = useState(true);

  useEffect(() => {
    if (visible && mode === 'edit' && operation) {
      form.setFieldsValue({
        name: operation.name,
        operation_type: operation.operation_type,
        risk_level: operation.risk_level,
        requires_approval: operation.requires_approval,
        approver_roles: operation.approver_roles || [],
        description: operation.description || '',
      });
      setRequiresApproval(operation.requires_approval);
    } else if (visible && mode === 'create') {
      form.resetFields();
      form.setFieldsValue({
        risk_level: 'medium',
        requires_approval: true,
      });
      setRequiresApproval(true);
    }
  }, [visible, mode, operation, form]);

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();
      setSubmitting(true);

      if (mode === 'create') {
        const payload: CreateSensitiveOperationRequest = {
          name: values.name,
          operation_type: values.operation_type,
          risk_level: values.risk_level,
          requires_approval: values.requires_approval,
          approver_roles: values.requires_approval ? (values.approver_roles || []) : [],
          description: values.description || undefined,
        };
        await apiClient.post('/sensitive-operations', payload);
        message.success('敏感操作创建成功');
      } else if (operation) {
        await apiClient.put(`/sensitive-operations/${operation.id}`, {
          name: values.name,
          operation_type: values.operation_type,
          risk_level: values.risk_level,
          requires_approval: values.requires_approval,
          approver_roles: values.requires_approval ? (values.approver_roles || []) : [],
          description: values.description || undefined,
        });
        message.success('敏感操作更新成功');
      }

      onSuccess();
    } catch (error: any) {
      if (error.response) {
        message.error(error.response.data?.error || '操作失败');
      }
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Modal
      title={mode === 'create' ? '创建敏感操作' : '编辑敏感操作'}
      open={visible}
      onCancel={onCancel}
      onOk={handleSubmit}
      confirmLoading={submitting}
      okText={mode === 'create' ? '创建' : '保存'}
      cancelText="取消"
      width={560}
      destroyOnClose
    >
      <Form form={form} layout="vertical">
        <Form.Item
          name="name"
          label="操作名称"
          rules={[
            { required: true, message: '请输入操作名称' },
            { min: 2, max: 100, message: '名称长度 2-100 字符' },
          ]}
        >
          <Input placeholder="例如：删除用户数据" maxLength={100} />
        </Form.Item>

        <Form.Item
          name="operation_type"
          label="操作类型"
          rules={[{ required: true, message: '请选择操作类型' }]}
        >
          <Select placeholder="选择操作类型">
            {SENSITIVE_OP_TYPE_OPTIONS.map((opt) => (
              <Option key={opt.value} value={opt.value}>
                <Space>
                  <span>{opt.label}</span>
                  <span style={{ color: '#999', fontSize: 12 }}>{opt.description}</span>
                </Space>
              </Option>
            ))}
          </Select>
        </Form.Item>

        <Form.Item
          name="risk_level"
          label="风险等级"
          rules={[{ required: true, message: '请选择风险等级' }]}
        >
          <Select placeholder="选择风险等级">
            {RISK_LEVEL_OPTIONS.map((opt) => (
              <Option key={opt.value} value={opt.value}>
                <Tag color={opt.color}>{opt.label}</Tag>
              </Option>
            ))}
          </Select>
        </Form.Item>

        <Form.Item
          name="requires_approval"
          label="是否需要审批"
          valuePropName="checked"
        >
          <Switch
            checkedChildren="需要审批"
            unCheckedChildren="自动放行"
            onChange={(checked) => setRequiresApproval(checked)}
          />
        </Form.Item>

        {requiresApproval && (
          <Form.Item
            name="approver_roles"
            label="审批角色"
            extra="选择有权审批此操作的角色"
          >
            <Select
              mode="tags"
              placeholder="输入角色名后按回车添加，如：admin、security_officer"
              tokenSeparators={[',']}
            />
          </Form.Item>
        )}

        <Form.Item name="description" label="描述">
          <TextArea
            placeholder="操作说明（可选）"
            maxLength={500}
            rows={3}
            showCount
          />
        </Form.Item>
      </Form>
    </Modal>
  );
};
