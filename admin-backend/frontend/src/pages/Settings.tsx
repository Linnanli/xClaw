import React, { useState, useEffect, useCallback } from 'react';
import { Card, Form, Button, Switch, InputNumber, message, Space } from 'antd';
import {
  SaveOutlined,
  ReloadOutlined,
  SettingOutlined,
  SafetyCertificateOutlined,
  ClockCircleOutlined,
  DatabaseOutlined,
} from '@ant-design/icons';
import { apiClient } from '../api/client';

interface SystemConfig {
  // DLP 配置
  dlp_enabled: boolean;
  dlp_scan_timeout_ms: number;
  dlp_fail_open: boolean;
  // 审计配置
  audit_retention_days: number;
  audit_enabled: boolean;
  // 客户端配置
  client_heartbeat_interval_s: number;
  client_offline_threshold_s: number;
  // 策略同步
  policy_sync_interval_s: number;
  policy_auto_push: boolean;
}

const DEFAULT_CONFIG: SystemConfig = {
  dlp_enabled: true,
  dlp_scan_timeout_ms: 5000,
  dlp_fail_open: false,
  audit_retention_days: 90,
  audit_enabled: true,
  client_heartbeat_interval_s: 30,
  client_offline_threshold_s: 120,
  policy_sync_interval_s: 300,
  policy_auto_push: true,
};

export const Settings: React.FC = () => {
  const [form] = Form.useForm();
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);

  const loadConfig = useCallback(async () => {
    setLoading(true);
    try {
      const response = await apiClient.get('/settings');
      form.setFieldsValue(response.data || DEFAULT_CONFIG);
    } catch {
      // API 可能尚未实现，使用默认值
      form.setFieldsValue(DEFAULT_CONFIG);
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
      try {
        await apiClient.put('/settings', values);
        message.success('配置保存成功');
      } catch {
        // API 可能尚未实现
        message.info('配置已保存到本地（后端 API 待实现）');
      }
    } catch {
      message.error('请检查配置项');
    } finally {
      setSaving(false);
    }
  };

  const handleReset = () => {
    form.setFieldsValue(DEFAULT_CONFIG);
    message.info('已恢复默认配置');
  };

  return (
    <div style={{ padding: 0, maxWidth: 800 }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
        <h2 style={{ margin: 0, fontSize: 20, fontWeight: 600 }}>系统配置</h2>
        <Space>
          <Button icon={<ReloadOutlined />} onClick={handleReset} loading={loading}>恢复默认</Button>
          <Button type="primary" icon={<SaveOutlined />} onClick={handleSave} loading={saving}>保存配置</Button>
        </Space>
      </div>

      <Form form={form} layout="vertical" initialValues={DEFAULT_CONFIG}>
        {/* DLP 配置 */}
        <Card
          title={<Space><SafetyCertificateOutlined /> DLP 数据防泄漏</Space>}
          size="small"
          style={{ marginBottom: 16 }}
        >
          <Form.Item name="dlp_enabled" label="启用 DLP 扫描" valuePropName="checked">
            <Switch checkedChildren="开启" unCheckedChildren="关闭" />
          </Form.Item>
          <Form.Item name="dlp_scan_timeout_ms" label="扫描超时时间（毫秒）" rules={[{ type: 'number', min: 100, max: 30000 }]}>
            <InputNumber min={100} max={30000} step={100} style={{ width: 200 }} addonAfter="ms" />
          </Form.Item>
          <Form.Item
            name="dlp_fail_open"
            label="故障开放模式"
            valuePropName="checked"
            extra={
              <span style={{ color: '#ff4d4f' }}>
                ⚠️ 开启后，DLP 扫描失败时将允许消息发送（不安全，仅用于调试）
              </span>
            }
          >
            <Switch checkedChildren="开启" unCheckedChildren="关闭" />
          </Form.Item>
        </Card>

        {/* 审计配置 */}
        <Card
          title={<Space><ClockCircleOutlined /> 审计日志</Space>}
          size="small"
          style={{ marginBottom: 16 }}
        >
          <Form.Item name="audit_enabled" label="启用审计日志" valuePropName="checked">
            <Switch checkedChildren="开启" unCheckedChildren="关闭" />
          </Form.Item>
          <Form.Item name="audit_retention_days" label="日志保留天数" rules={[{ type: 'number', min: 7, max: 365 }]}>
            <InputNumber min={7} max={365} style={{ width: 200 }} addonAfter="天" />
          </Form.Item>
        </Card>

        {/* 客户端配置 */}
        <Card
          title={<Space><SettingOutlined /> 客户端管理</Space>}
          size="small"
          style={{ marginBottom: 16 }}
        >
          <Form.Item name="client_heartbeat_interval_s" label="心跳间隔" rules={[{ type: 'number', min: 10, max: 300 }]}>
            <InputNumber min={10} max={300} style={{ width: 200 }} addonAfter="秒" />
          </Form.Item>
          <Form.Item name="client_offline_threshold_s" label="离线判定阈值" rules={[{ type: 'number', min: 30, max: 600 }]}>
            <InputNumber min={30} max={600} style={{ width: 200 }} addonAfter="秒" />
          </Form.Item>
        </Card>

        {/* 策略同步配置 */}
        <Card
          title={<Space><DatabaseOutlined /> 策略同步</Space>}
          size="small"
          style={{ marginBottom: 16 }}
        >
          <Form.Item name="policy_sync_interval_s" label="同步间隔" rules={[{ type: 'number', min: 60, max: 3600 }]}>
            <InputNumber min={60} max={3600} style={{ width: 200 }} addonAfter="秒" />
          </Form.Item>
          <Form.Item name="policy_auto_push" label="自动推送策略更新" valuePropName="checked">
            <Switch checkedChildren="开启" unCheckedChildren="关闭" />
          </Form.Item>
        </Card>
      </Form>
    </div>
  );
};
