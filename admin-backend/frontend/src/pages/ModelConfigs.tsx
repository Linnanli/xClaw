import React, { useState, useEffect, useCallback } from 'react';
import {
  Table, Button, Tag, Input, Empty, Card, Row, Col, Statistic,
  Switch, message, Modal, Form, Space, Popconfirm, Tooltip,
} from 'antd';
import {
  ReloadOutlined,
  SearchOutlined,
  PlusOutlined,
  EditOutlined,
  DeleteOutlined,
  RobotOutlined,
  StarOutlined,
  EyeInvisibleOutlined,
  CopyOutlined,
} from '@ant-design/icons';
import type { ColumnsType } from 'antd/es/table';
import { apiClient } from '../api/client';

interface ModelConfig {
  id: string;
  model_id: string;
  display_name: string;
  description: string | null;
  provider: string;
  api_base_url: string | null;
  api_key: string | null;
  enabled: boolean;
  is_default: boolean;
  sort_order: number;
  capabilities: string[];
  extra_config: Record<string, unknown>;
  created_at: string;
  updated_at: string;
}

interface ModelFormValues {
  model_id: string;
  display_name: string;
  description?: string;
  provider: string;
  api_base_url?: string;
  api_key?: string;
  sort_order?: number;
  capabilities?: string;
}

const PROVIDER_COLORS: Record<string, string> = {
  openai: 'green',
  anthropic: 'purple',
  deepseek: 'blue',
  custom: 'orange',
};

export const ModelConfigs: React.FC = () => {
  const [models, setModels] = useState<ModelConfig[]>([]);
  const [loading, setLoading] = useState(false);
  const [searchText, setSearchText] = useState('');
  const [modalOpen, setModalOpen] = useState(false);
  const [editingModel, setEditingModel] = useState<ModelConfig | null>(null);
  const [saving, setSaving] = useState(false);
  const [form] = Form.useForm<ModelFormValues>();

  const loadModels = useCallback(async () => {
    setLoading(true);
    try {
      const response = await apiClient.get('/model-configs');
      setModels(Array.isArray(response.data) ? response.data : []);
    } catch {
      setModels([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadModels();
  }, [loadModels]);

  const filteredModels = models.filter((m) => {
    if (!searchText) return true;
    const kw = searchText.toLowerCase();
    return (
      m.model_id.toLowerCase().includes(kw) ||
      m.display_name.toLowerCase().includes(kw) ||
      m.provider.toLowerCase().includes(kw)
    );
  });

  const handleCreate = () => {
    setEditingModel(null);
    form.resetFields();
    setModalOpen(true);
  };

  const handleEdit = (record: ModelConfig) => {
    setEditingModel(record);
    form.setFieldsValue({
      model_id: record.model_id,
      display_name: record.display_name,
      description: record.description || undefined,
      provider: record.provider,
      api_base_url: record.api_base_url || undefined,
      api_key: undefined, // 不回显 API Key
      sort_order: record.sort_order,
      capabilities: Array.isArray(record.capabilities)
        ? record.capabilities.join(', ')
        : '',
    });
    setModalOpen(true);
  };

  const handleDelete = async (id: string) => {
    try {
      await apiClient.delete(`/model-configs/${id}`);
      message.success('模型配置已删除');
      loadModels();
    } catch (err: any) {
      message.error(err?.response?.data?.error || '删除失败');
    }
  };

  const handleToggleEnabled = async (record: ModelConfig) => {
    try {
      await apiClient.put(`/model-configs/${record.id}`, {
        enabled: !record.enabled,
      });
      message.success(`${record.display_name} 已${record.enabled ? '禁用' : '启用'}`);
      loadModels();
    } catch {
      message.error('操作失败');
    }
  };

  const handleSetDefault = async (record: ModelConfig) => {
    try {
      await apiClient.put(`/model-configs/${record.id}`, {
        is_default: true,
      });
      message.success(`${record.display_name} 已设为默认模型`);
      loadModels();
    } catch {
      message.error('操作失败');
    }
  };

  const handleSave = async () => {
    try {
      const values = await form.validateFields();
      setSaving(true);

      const payload: Record<string, unknown> = {
        model_id: values.model_id,
        display_name: values.display_name,
        description: values.description || null,
        provider: values.provider,
        api_base_url: values.api_base_url || null,
        sort_order: values.sort_order ?? 0,
        capabilities: values.capabilities
          ? values.capabilities.split(',').map((s: string) => s.trim()).filter(Boolean)
          : [],
      };

      // 仅在填写了 API Key 时才发送
      if (values.api_key) {
        payload.api_key = values.api_key;
      }

      if (editingModel) {
        await apiClient.put(`/model-configs/${editingModel.id}`, payload);
        message.success('模型配置已更新');
      } else {
        await apiClient.post('/model-configs', payload);
        message.success('模型配置已创建');
      }

      setModalOpen(false);
      form.resetFields();
      loadModels();
    } catch (err: any) {
      if (err?.response?.data?.error) {
        message.error(err.response.data.error);
      }
      // form validation error — ignore
    } finally {
      setSaving(false);
    }
  };

  const maskApiKey = (key: string | null): string => {
    if (!key) return '-';
    if (key.length <= 8) return '••••••••';
    return `${key.slice(0, 4)}••••${key.slice(-4)}`;
  };

  const columns: ColumnsType<ModelConfig> = [
    {
      title: '排序',
      dataIndex: 'sort_order',
      key: 'sort_order',
      width: 60,
      sorter: (a, b) => a.sort_order - b.sort_order,
    },
    {
      title: '模型名称',
      dataIndex: 'display_name',
      key: 'display_name',
      width: 180,
      render: (name: string, record: ModelConfig) => (
        <Space>
          <span style={{ fontWeight: 500 }}>{name}</span>
          {record.is_default && <Tag color="gold"><StarOutlined /> 默认</Tag>}
        </Space>
      ),
    },
    {
      title: '模型 ID',
      dataIndex: 'model_id',
      key: 'model_id',
      width: 200,
      render: (id: string) => <code style={{ fontSize: 12 }}>{id}</code>,
    },
    {
      title: '提供商',
      dataIndex: 'provider',
      key: 'provider',
      width: 120,
      render: (provider: string) => (
        <Tag color={PROVIDER_COLORS[provider] || 'default'}>{provider}</Tag>
      ),
    },
    {
      title: 'API Endpoint',
      dataIndex: 'api_base_url',
      key: 'api_base_url',
      ellipsis: true,
      render: (url: string | null) =>
        url ? (
          <Tooltip title={url}>
            <span style={{ fontSize: 12 }}>{url}</span>
          </Tooltip>
        ) : (
          <span style={{ color: '#bfbfbf' }}>-</span>
        ),
    },
    {
      title: 'API Key',
      dataIndex: 'api_key',
      key: 'api_key',
      width: 150,
      render: (key: string | null) => (
        <Space size={4}>
          <EyeInvisibleOutlined style={{ color: '#bfbfbf' }} />
          <span style={{ fontSize: 12, color: '#999' }}>{maskApiKey(key)}</span>
        </Space>
      ),
    },
    {
      title: '能力',
      dataIndex: 'capabilities',
      key: 'capabilities',
      width: 160,
      render: (caps: string[]) =>
        Array.isArray(caps) && caps.length > 0
          ? caps.map((c) => <Tag key={c} style={{ fontSize: 11 }}>{c}</Tag>)
          : <span style={{ color: '#bfbfbf' }}>-</span>,
    },
    {
      title: '状态',
      dataIndex: 'enabled',
      key: 'enabled',
      width: 80,
      render: (enabled: boolean, record: ModelConfig) => (
        <Switch
          checked={enabled}
          onChange={() => handleToggleEnabled(record)}
          checkedChildren="启用"
          unCheckedChildren="禁用"
          size="small"
        />
      ),
    },
    {
      title: '操作',
      key: 'actions',
      width: 180,
      render: (_: unknown, record: ModelConfig) => (
        <Space size={4}>
          {!record.is_default && (
            <Tooltip title="设为默认">
              <Button
                type="text"
                size="small"
                icon={<StarOutlined />}
                onClick={() => handleSetDefault(record)}
              />
            </Tooltip>
          )}
          <Tooltip title="编辑">
            <Button
              type="text"
              size="small"
              icon={<EditOutlined />}
              onClick={() => handleEdit(record)}
            />
          </Tooltip>
          <Popconfirm
            title="确定删除此模型配置？"
            description={record.is_default ? '⚠️ 这是默认模型，删除后需要重新设置默认模型' : undefined}
            onConfirm={() => handleDelete(record.id)}
            okText="删除"
            cancelText="取消"
            okButtonProps={{ danger: true }}
          >
            <Tooltip title="删除">
              <Button type="text" size="small" danger icon={<DeleteOutlined />} />
            </Tooltip>
          </Popconfirm>
        </Space>
      ),
    },
  ];

  const enabledCount = models.filter((m) => m.enabled).length;

  return (
    <div style={{ padding: 0 }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
        <h2 style={{ margin: 0, fontSize: 20, fontWeight: 600 }}>模型配置</h2>
        <Space>
          <Button icon={<ReloadOutlined />} onClick={loadModels} loading={loading}>刷新</Button>
          <Button type="primary" icon={<PlusOutlined />} onClick={handleCreate}>添加模型</Button>
        </Space>
      </div>

      <Row gutter={16} style={{ marginBottom: 16 }}>
        <Col span={6}>
          <Card size="small">
            <Statistic title="模型总数" value={models.length} prefix={<RobotOutlined />} />
          </Card>
        </Col>
        <Col span={6}>
          <Card size="small">
            <Statistic title="已启用" value={enabledCount} valueStyle={{ color: '#3f8600' }} />
          </Card>
        </Col>
        <Col span={6}>
          <Card size="small">
            <Statistic title="已禁用" value={models.length - enabledCount} valueStyle={{ color: '#cf1322' }} />
          </Card>
        </Col>
        <Col span={6}>
          <Card size="small">
            <Statistic
              title="提供商"
              value={new Set(models.map((m) => m.provider)).size}
            />
          </Card>
        </Col>
      </Row>

      <div style={{ display: 'flex', gap: 12, marginBottom: 16 }}>
        <Input
          placeholder="搜索模型名称、ID 或提供商"
          prefix={<SearchOutlined />}
          value={searchText}
          onChange={(e) => setSearchText(e.target.value)}
          allowClear
          style={{ width: 320 }}
        />
        <span style={{ color: '#999', fontSize: 13, marginLeft: 'auto', lineHeight: '32px' }}>
          共 {models.length} 个模型配置
        </span>
      </div>

      <Table
        columns={columns}
        dataSource={filteredModels}
        rowKey="id"
        loading={loading}
        locale={{
          emptyText: (
            <Empty
              image={<RobotOutlined style={{ fontSize: 48, color: '#bfbfbf' }} />}
              description="暂无模型配置，点击「添加模型」创建第一个"
            />
          ),
        }}
        pagination={
          filteredModels.length > 10
            ? { pageSize: 10, showSizeChanger: true, showTotal: (t) => `共 ${t} 个模型` }
            : false
        }
        size="middle"
        scroll={{ x: 1200 }}
      />

      {/* 创建/编辑弹窗 */}
      <Modal
        title={editingModel ? '编辑模型配置' : '添加模型配置'}
        open={modalOpen}
        onCancel={() => { setModalOpen(false); form.resetFields(); }}
        onOk={handleSave}
        confirmLoading={saving}
        okText={editingModel ? '保存' : '创建'}
        cancelText="取消"
        width={560}
        destroyOnClose
      >
        <Form form={form} layout="vertical" style={{ marginTop: 16 }}>
          <Form.Item
            name="model_id"
            label="模型 ID"
            rules={[{ required: true, message: '请输入模型 ID' }]}
            extra="模型提供商的标识符，如 gpt-4o、claude-3-5-sonnet-20241022"
          >
            <Input placeholder="gpt-4o" disabled={!!editingModel} />
          </Form.Item>

          <Form.Item
            name="display_name"
            label="显示名称"
            rules={[{ required: true, message: '请输入显示名称' }]}
          >
            <Input placeholder="GPT-4o" />
          </Form.Item>

          <Form.Item name="description" label="描述">
            <Input.TextArea placeholder="模型描述（可选）" rows={2} />
          </Form.Item>

          <Form.Item
            name="provider"
            label="提供商"
            rules={[{ required: true, message: '请输入提供商' }]}
          >
            <Input placeholder="openai" />
          </Form.Item>

          <Form.Item name="api_base_url" label="API Endpoint">
            <Input placeholder="https://api.openai.com/v1" />
          </Form.Item>

          <Form.Item
            name="api_key"
            label="API Key"
            extra={editingModel ? '留空则保持原有 Key 不变' : undefined}
          >
            <Input.Password placeholder="sk-..." />
          </Form.Item>

          <Row gutter={16}>
            <Col span={12}>
              <Form.Item name="sort_order" label="排序权重">
                <InputNumber min={0} max={999} style={{ width: '100%' }} placeholder="0" />
              </Form.Item>
            </Col>
            <Col span={12}>
              <Form.Item
                name="capabilities"
                label="能力标签"
                extra="逗号分隔，如 chat, vision, code"
              >
                <Input placeholder="chat, vision" />
              </Form.Item>
            </Col>
          </Row>
        </Form>
      </Modal>
    </div>
  );
};
