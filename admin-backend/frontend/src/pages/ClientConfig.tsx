import React, { useState, useEffect, useCallback } from 'react';
import { Card, Form, Input, Button, Switch, InputNumber, message, Space, Tag, Spin } from 'antd';
import {
  SaveOutlined,
  ReloadOutlined,
  CloudServerOutlined,
  KeyOutlined,
  SafetyCertificateOutlined,
} from '@ant-design/icons';
import { apiClient } from '../api/client';

interface ClientConfig {
  llm_backend: string | null;
  llm_api_key: string | null;
  llm_model: string | null;
  llm_base_url: string | null;
  safety_enabled: boolean | null;
  skills_enabled: boolean | null;
  extensions_enabled: boolean | null;
  max_cost_per_day_cents: number | null;
  config_version: number;
  updated_at: string;
}

export const ClientConfig: React.FC = () => {
  const [form] = Form.useForm();
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [configVersion, setConfigVersion] = useState(0);

  const loadConfig = useCallback(async () => {
    setLoading(true);
    try {
      const response = await apiClient.get<ClientConfig>('/client-config');
      const data = response.data;
      form.setFieldsValue({
        llm_backend: data.llm_backend || '',
        llm_api_key: '',
        llm_model: data.llm_model || '',
        llm_base_url: data.llm_base_url || '',
        safety_enabled: data.safety_enabled ?? true,
        skills_enabled: data.skills_enabled ?? true,
        extensions_enabled: data.extensions_enabled ?? true,
        max_cost_per_day_cents: data.max_cost_per_day_cents ?? 0,
      });
      setConfigVersion(data.config_version);
    } catch {
      message.error('加载配置失败');
    } finally {
      setLoading(false);
    }
  }, [form]);

  useEffect(() => {
    loadConfig();
  }, [loadConfig]);

  const handleSave = async () => {
    try {
      const values = await form.validateFields();
      setSaving(true);

      // 构建请求体：只发送有值的字段
      const payload: Record<string, unknown> = {};
      if (values.llm_backend) payload.llm_backend = values.llm_backend;
      if (values.llm_api_key) payload.llm_api_key = values.llm_api_key;
      if (values.llm_model) payload.llm_model = values.llm_model;
      if (values.llm_base_url !== undefined) payload.llm_base_url = values.llm_base_url;
      payload.safety_enabled = values.safety_enabled;
      payload.skills_enabled = values.skills_enabled;
      payload.extensions_enabled = values.extensions_enabled;
      if (values.max_cost_per_day_cents !== undefined && values.max_cost_per_day_cents !== null) {
        payload.max_cost_per_day_cents = values.max_cost_per_day_cents;
      }

      const response = await apiClient.put('/client-config', payload);
      message.success(`配置保存成功 (v${response.data.config_version})`);
      setConfigVersion(response.data.config_version);
      // 清空 API Key 输入框（已保存）
      form.setFieldValue('llm_api_key', '');
    } catch (err: unknown) {
      const error = err as { response?: { data?: { details?: string } } };
      message.error(error.response?.data?.details || '保存配置失败');
    } finally {
      setSaving(false);
    }
  };

  return (
    <Spin spinning={loading}>
      <div style={{ padding: 0, maxWidth: 800 }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
          <Space>
            <h2 style={{ margin: 0, fontSize: 20, fontWeight: 600 }}>客户端配置</h2>
            <Tag color="blue">v{configVersion}</Tag>
          </Space>
          <Space>
            <Button icon={<ReloadOutlined />} onClick={loadConfig}>刷新</Button>
            <Button type="primary" icon={<SaveOutlined />} onClick={handleSave} loading={saving}>保存配置</Button>
          </Space>
        </div>

        <Form form={form} layout="vertical">
          <Card
            title={<Space><CloudServerOutlined /> LLM 后端配置</Space>}
            size="small"
            style={{ marginBottom: 16 }}
          >
            <Form.Item name="llm_backend" label="LLM 后端">
              <Input placeholder="例如: openai, anthropic, local" />
            </Form.Item>
            <Form.Item
              name="llm_api_key"
              label="API Key"
              extra="留空表示不修改现有 Key。当前值已脱敏显示。"
            >
              <Input.Password placeholder="输入新的 API Key（留空不修改）" />
            </Form.Item>
            <Form.Item name="llm_model" label="模型名称">
              <Input placeholder="例如: gpt-4, claude-3-opus" />
            </Form.Item>
            <Form.Item
              name="llm_base_url"
              label="Base URL"
              rules={[
                {
                  validator: (_, value) => {
                    if (!value || value === '' || value.startsWith('http://') || value.startsWith('https://')) {
                      return Promise.resolve();
                    }
                    return Promise.reject(new Error('URL 需以 http:// 或 https:// 开头'));
                  },
                },
              ]}
            >
              <Input placeholder="例如: https://api.openai.com/v1" />
            </Form.Item>
          </Card>

          <Card
            title={<Space><SafetyCertificateOutlined /> 功能开关</Space>}
            size="small"
            style={{ marginBottom: 16 }}
          >
            <Form.Item name="safety_enabled" label="安全防护" valuePropName="checked">
              <Switch checkedChildren="开启" unCheckedChildren="关闭" />
            </Form.Item>
            <Form.Item name="skills_enabled" label="技能系统" valuePropName="checked">
              <Switch checkedChildren="开启" unCheckedChildren="关闭" />
            </Form.Item>
            <Form.Item name="extensions_enabled" label="扩展系统" valuePropName="checked">
              <Switch checkedChildren="开启" unCheckedChildren="关闭" />
            </Form.Item>
          </Card>

          <Card
            title={<Space><KeyOutlined /> 费用控制</Space>}
            size="small"
            style={{ marginBottom: 16 }}
          >
            <Form.Item
              name="max_cost_per_day_cents"
              label="每日费用限制（分）"
              rules={[{ type: 'number', min: 0, message: '费用限制不能为负数' }]}
            >
              <InputNumber min={0} style={{ width: 200 }} addonAfter="分" />
            </Form.Item>
          </Card>
        </Form>
      </div>
    </Spin>
  );
};
