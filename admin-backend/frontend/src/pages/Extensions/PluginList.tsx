import React, { useState, useEffect, useCallback } from 'react';
import { Table, Button, Tag, Input, Empty, Card, Row, Col, Statistic, Switch, message } from 'antd';
import {
  ReloadOutlined,
  SearchOutlined,
  ApiOutlined,
  AppstoreAddOutlined,
} from '@ant-design/icons';
import type { ColumnsType } from 'antd/es/table';
import { apiClient } from '../../api/client';

interface Plugin {
  id: string;
  name: string;
  description?: string;
  version: string;
  author?: string;
  enabled: boolean;
  created_at: string;
  updated_at: string;
}

export const PluginList: React.FC = () => {
  const [plugins, setPlugins] = useState<Plugin[]>([]);
  const [loading, setLoading] = useState(false);
  const [searchText, setSearchText] = useState('');

  const loadPlugins = useCallback(async () => {
    setLoading(true);
    try {
      const response = await apiClient.get('/plugins');
      setPlugins(Array.isArray(response.data.plugins) ? response.data.plugins : Array.isArray(response.data) ? response.data : []);
    } catch {
      setPlugins([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadPlugins();
  }, [loadPlugins]);

  const filteredPlugins = plugins.filter((p) => {
    if (!searchText) return true;
    const kw = searchText.toLowerCase();
    return p.name.toLowerCase().includes(kw) || (p.description || '').toLowerCase().includes(kw);
  });

  const handleToggle = useCallback(async (plugin: Plugin) => {
    const action = plugin.enabled ? 'disable' : 'enable';
    try {
      await apiClient.post(`/plugins/${plugin.id}/${action}`);
      message.success(`${plugin.name} 已${plugin.enabled ? '禁用' : '启用'}`);
      loadPlugins();
    } catch {
      message.error(`操作失败`);
    }
  }, [loadPlugins]);

  const columns: ColumnsType<Plugin> = [
    {
      title: '插件名称',
      dataIndex: 'name',
      key: 'name',
      width: 200,
      render: (name: string) => <span style={{ fontWeight: 500 }}>{name}</span>,
    },
    {
      title: '描述',
      dataIndex: 'description',
      key: 'description',
      ellipsis: true,
      render: (desc: string | undefined) => desc || <span style={{ color: '#bfbfbf' }}>-</span>,
    },
    {
      title: '版本',
      dataIndex: 'version',
      key: 'version',
      width: 100,
      render: (v: string) => <Tag color="green">{v}</Tag>,
    },
    {
      title: '作者',
      dataIndex: 'author',
      key: 'author',
      width: 120,
      render: (author: string | undefined) => author || <span style={{ color: '#bfbfbf' }}>-</span>,
    },
    {
      title: '状态',
      dataIndex: 'enabled',
      key: 'enabled',
      width: 100,
      render: (enabled: boolean, record: Plugin) => (
        <Switch checked={enabled} onChange={() => handleToggle(record)} checkedChildren="启用" unCheckedChildren="禁用" />
      ),
    },
    {
      title: '更新时间',
      dataIndex: 'updated_at',
      key: 'updated_at',
      width: 170,
      render: (date: string) => new Date(date).toLocaleString('zh-CN', { year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' }),
    },
  ];

  return (
    <div style={{ padding: 0 }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
        <h2 style={{ margin: 0, fontSize: 20, fontWeight: 600 }}>插件管理</h2>
        <Button icon={<ReloadOutlined />} onClick={loadPlugins} loading={loading}>刷新</Button>
      </div>

      <Row gutter={16} style={{ marginBottom: 16 }}>
        <Col span={8}>
          <Card size="small">
            <Statistic title="已安装插件" value={plugins.length} prefix={<ApiOutlined />} />
          </Card>
        </Col>
      </Row>

      <div style={{ display: 'flex', gap: 12, marginBottom: 16 }}>
        <Input
          placeholder="搜索插件名称或描述"
          prefix={<SearchOutlined />}
          value={searchText}
          onChange={(e) => setSearchText(e.target.value)}
          allowClear
          style={{ width: 280 }}
        />
        <span style={{ color: '#999', fontSize: 13, marginLeft: 'auto', lineHeight: '32px' }}>
          共 {plugins.length} 个插件
        </span>
      </div>

      <Table
        columns={columns}
        dataSource={filteredPlugins}
        rowKey="id"
        loading={loading}
        locale={{
          emptyText: (
            <Empty
              image={<AppstoreAddOutlined style={{ fontSize: 48, color: '#bfbfbf' }} />}
              description="暂无已安装的插件，插件将通过主项目自动同步"
            />
          ),
        }}
        pagination={filteredPlugins.length > 10 ? { pageSize: 10, showSizeChanger: true, showTotal: (t) => `共 ${t} 个插件` } : false}
        size="middle"
      />
    </div>
  );
};
