import React, { useState, useEffect } from 'react';
import { Table, Button, Space, message, Popconfirm, Tag, Switch } from 'antd';
import { PlusOutlined, DeleteOutlined, ReloadOutlined } from '@ant-design/icons';
import type { ColumnsType } from 'antd/es/table';
import { CreateDlpRuleModal } from '../../components/Security/CreateDlpRuleModal';
import { apiClient } from '../../api/client';
import type { DlpRule } from '../../types';
import { getSeverityColor, getSeverityText, getCategoryText } from '../../constants/dlp';
import '../../styles/DlpRuleList.css';

export const DlpRuleList: React.FC = () => {
  const [rules, setRules] = useState<DlpRule[]>([]);
  const [loading, setLoading] = useState(false);
  const [createModalVisible, setCreateModalVisible] = useState(false);

  // 加载规则列表
  const loadRules = async () => {
    setLoading(true);
    try {
      const response = await apiClient.get('/dlp-rules');
      setRules(response.data.rules || []);
    } catch (error: any) {
      message.error(error.response?.data?.error || '加载 DLP 规则失败');
    } finally {
      setLoading(false);
    }
  };

  // 删除规则
  const handleDelete = async (ruleId: string) => {
    try {
      await apiClient.delete(`/dlp-rules/${ruleId}`);
      message.success('删除规则成功');
      loadRules();
    } catch (error: any) {
      message.error(error.response?.data?.error || '删除规则失败');
    }
  };

  // 切换规则状态
  const handleToggleStatus = async (ruleId: string, enabled: boolean) => {
    try {
      await apiClient.put(`/dlp-rules/${ruleId}`, { enabled });
      message.success(enabled ? '规则已启用' : '规则已禁用');
      loadRules();
    } catch (error: any) {
      message.error(error.response?.data?.error || '更新规则状态失败');
    }
  };

  // 创建规则成功回调
  const handleCreateSuccess = () => {
    setCreateModalVisible(false);
    loadRules();
  };

  useEffect(() => {
    loadRules();
  }, []);

  // 表格列配置
  const columns: ColumnsType<DlpRule> = [
    {
      title: '规则名',
      dataIndex: 'name',
      key: 'name',
      width: 150,
    },
    {
      title: '匹配模式',
      dataIndex: 'pattern',
      key: 'pattern',
      width: 200,
      ellipsis: true,
    },
    {
      title: '严重级别',
      dataIndex: 'severity',
      key: 'severity',
      width: 100,
      render: (severity: string) => (
        <Tag color={getSeverityColor(severity)}>
          {getSeverityText(severity)}
        </Tag>
      ),
    },
    {
      title: '分类',
      dataIndex: 'category',
      key: 'category',
      width: 120,
      render: (category: string) => getCategoryText(category),
    },
    {
      title: '描述',
      dataIndex: 'description',
      key: 'description',
      width: 200,
      ellipsis: true,
    },
    {
      title: '状态',
      dataIndex: 'enabled',
      key: 'enabled',
      width: 100,
      render: (enabled: boolean, record) => (
        <Switch
          checked={enabled}
          onChange={(checked) => handleToggleStatus(record.id, checked)}
          checkedChildren="启用"
          unCheckedChildren="禁用"
        />
      ),
    },
    {
      title: '创建时间',
      dataIndex: 'created_at',
      key: 'created_at',
      width: 180,
      render: (date: string) => new Date(date).toLocaleString('zh-CN'),
    },
    {
      title: '操作',
      key: 'action',
      width: 120,
      fixed: 'right',
      render: (_, record) => (
        <Space size="small">
          <Popconfirm
            title="确认删除"
            description={`确定要删除规则 "${record.name}" 吗？`}
            onConfirm={() => handleDelete(record.id)}
            okText="确定"
            cancelText="取消"
          >
            <Button
              type="link"
              danger
              size="small"
              icon={<DeleteOutlined />}
            >
              删除
            </Button>
          </Popconfirm>
        </Space>
      ),
    },
  ];

  return (
    <div className="dlp-rule-list-container">
      <div className="page-header">
        <h2>DLP 规则管理</h2>
        <Space>
          <Button
            icon={<ReloadOutlined />}
            onClick={loadRules}
            loading={loading}
          >
            刷新
          </Button>
          <Button
            type="primary"
            icon={<PlusOutlined />}
            onClick={() => setCreateModalVisible(true)}
          >
            创建规则
          </Button>
        </Space>
      </div>

      <Table
        columns={columns}
        dataSource={rules}
        rowKey="id"
        loading={loading}
        pagination={{
          pageSize: 10,
          showSizeChanger: true,
          showTotal: (total) => `共 ${total} 条规则`,
        }}
        scroll={{ x: 1200 }}
      />

      <CreateDlpRuleModal
        visible={createModalVisible}
        onCancel={() => setCreateModalVisible(false)}
        onSuccess={handleCreateSuccess}
      />
    </div>
  );
};
