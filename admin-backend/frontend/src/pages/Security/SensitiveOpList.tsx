import React, { useState, useEffect, useMemo, useCallback } from 'react';
import { Table, Button, Space, message, Popconfirm, Tag, Switch, Input, Select, Empty } from 'antd';
import {
  PlusOutlined,
  DeleteOutlined,
  EditOutlined,
  ReloadOutlined,
  SearchOutlined,
  WarningOutlined,
} from '@ant-design/icons';
import type { ColumnsType } from 'antd/es/table';
import { SensitiveOpFormModal, type SensitiveOpFormMode } from '../../components/Security/SensitiveOpFormModal';
import { apiClient } from '../../api/client';
import type { SensitiveOperation } from '../../types';
import {
  getOpTypeText,
  getRiskLevelText,
  getRiskLevelColor,
  SENSITIVE_OP_TYPE_OPTIONS,
  RISK_LEVEL_OPTIONS,
} from '../../constants/sensitiveOps';
import '../../styles/SensitiveOpList.css';

const { Option } = Select;

export const SensitiveOpList: React.FC = () => {
  const [operations, setOperations] = useState<SensitiveOperation[]>([]);
  const [loading, setLoading] = useState(false);
  const [modalVisible, setModalVisible] = useState(false);
  const [modalMode, setModalMode] = useState<SensitiveOpFormMode>('create');
  const [editingOp, setEditingOp] = useState<SensitiveOperation | null>(null);

  // 搜索和筛选
  const [searchText, setSearchText] = useState('');
  const [typeFilter, setTypeFilter] = useState<string | undefined>(undefined);
  const [riskFilter, setRiskFilter] = useState<string | undefined>(undefined);
  const [statusFilter, setStatusFilter] = useState<string | undefined>(undefined);

  const loadOperations = useCallback(async () => {
    setLoading(true);
    try {
      const response = await apiClient.get('/sensitive-operations');
      setOperations(response.data.operations || []);
    } catch (error: any) {
      message.error(error.response?.data?.error || '加载敏感操作列表失败');
    } finally {
      setLoading(false);
    }
  }, []);

  const handleDelete = useCallback(async (id: string) => {
    try {
      await apiClient.delete(`/sensitive-operations/${id}`);
      message.success('删除成功');
      loadOperations();
    } catch (error: any) {
      message.error(error.response?.data?.error || '删除失败');
    }
  }, [loadOperations]);

  const handleToggleStatus = useCallback(async (id: string, enabled: boolean) => {
    try {
      await apiClient.put(`/sensitive-operations/${id}`, { enabled });
      message.success(enabled ? '已启用' : '已禁用');
      loadOperations();
    } catch (error: any) {
      message.error(error.response?.data?.error || '更新状态失败');
    }
  }, [loadOperations]);

  const handleCreate = useCallback(() => {
    setModalMode('create');
    setEditingOp(null);
    setModalVisible(true);
  }, []);

  const handleEdit = useCallback((op: SensitiveOperation) => {
    setModalMode('edit');
    setEditingOp(op);
    setModalVisible(true);
  }, []);

  const handleFormSuccess = useCallback(() => {
    setModalVisible(false);
    setEditingOp(null);
    loadOperations();
  }, [loadOperations]);

  const handleFormCancel = useCallback(() => {
    setModalVisible(false);
    setEditingOp(null);
  }, []);

  useEffect(() => {
    loadOperations();
  }, [loadOperations]);

  // 前端筛选
  const filteredOps = useMemo(() => {
    return operations.filter((op) => {
      if (searchText) {
        const kw = searchText.toLowerCase();
        const matchesSearch =
          op.name.toLowerCase().includes(kw) ||
          (op.description || '').toLowerCase().includes(kw);
        if (!matchesSearch) return false;
      }
      if (typeFilter && op.operation_type !== typeFilter) return false;
      if (riskFilter && op.risk_level !== riskFilter) return false;
      if (statusFilter === 'enabled' && !op.enabled) return false;
      if (statusFilter === 'disabled' && op.enabled) return false;
      return true;
    });
  }, [operations, searchText, typeFilter, riskFilter, statusFilter]);

  const hasActiveFilters = searchText || typeFilter || riskFilter || statusFilter;

  const clearFilters = useCallback(() => {
    setSearchText('');
    setTypeFilter(undefined);
    setRiskFilter(undefined);
    setStatusFilter(undefined);
  }, []);

  const columns: ColumnsType<SensitiveOperation> = [
    {
      title: '操作名称',
      dataIndex: 'name',
      key: 'name',
      width: 180,
      render: (name: string, record) => (
        <Button type="link" onClick={() => handleEdit(record)} style={{ padding: 0, fontWeight: 500 }}>
          {name}
        </Button>
      ),
    },
    {
      title: '操作类型',
      dataIndex: 'operation_type',
      key: 'operation_type',
      width: 120,
      render: (type: string) => <Tag>{getOpTypeText(type)}</Tag>,
    },
    {
      title: '风险等级',
      dataIndex: 'risk_level',
      key: 'risk_level',
      width: 100,
      render: (level: string) => (
        <Tag color={getRiskLevelColor(level)}>{getRiskLevelText(level)}</Tag>
      ),
    },
    {
      title: '审批要求',
      dataIndex: 'requires_approval',
      key: 'requires_approval',
      width: 120,
      render: (requires: boolean, record) => (
        requires ? (
          <Space size={4}>
            <Tag color="warning">需要审批</Tag>
            {record.approver_roles && record.approver_roles.length > 0 && (
              <span style={{ color: '#999', fontSize: 12 }}>
                ({record.approver_roles.join(', ')})
              </span>
            )}
          </Space>
        ) : (
          <Tag color="success">自动放行</Tag>
        )
      ),
    },
    {
      title: '描述',
      dataIndex: 'description',
      key: 'description',
      ellipsis: true,
      render: (desc: string | undefined) => desc || <span style={{ color: '#bfbfbf' }}>-</span>,
    },
    {
      title: '状态',
      dataIndex: 'enabled',
      key: 'enabled',
      width: 90,
      render: (enabled: boolean, record) => (
        <Switch
          checked={enabled}
          onChange={(checked) => handleToggleStatus(record.id, checked)}
          checkedChildren="启用"
          unCheckedChildren="禁用"
          size="small"
        />
      ),
    },
    {
      title: '更新时间',
      dataIndex: 'updated_at',
      key: 'updated_at',
      width: 160,
      render: (date: string) => formatRelativeTime(date),
    },
    {
      title: '操作',
      key: 'action',
      width: 140,
      fixed: 'right',
      render: (_, record) => (
        <Space size="small">
          <Button type="link" size="small" icon={<EditOutlined />} onClick={() => handleEdit(record)}>
            编辑
          </Button>
          <Popconfirm
            title="确认删除"
            description={`确定要删除「${record.name}」吗？删除后不可恢复。`}
            onConfirm={() => handleDelete(record.id)}
            okText="确定"
            cancelText="取消"
            okButtonProps={{ danger: true }}
          >
            <Button type="link" danger size="small" icon={<DeleteOutlined />}>
              删除
            </Button>
          </Popconfirm>
        </Space>
      ),
    },
  ];

  const emptyContent = operations.length === 0 ? (
    <Empty
      image={<WarningOutlined style={{ fontSize: 48, color: '#bfbfbf' }} />}
      description={
        <span>
          还没有敏感操作规则，<Button type="link" onClick={handleCreate} style={{ padding: 0 }}>创建第一条规则</Button> 来管控高风险操作
        </span>
      }
    />
  ) : hasActiveFilters ? (
    <Empty
      description={
        <span>
          没有匹配的规则，<Button type="link" onClick={clearFilters} style={{ padding: 0 }}>清除筛选条件</Button>
        </span>
      }
    />
  ) : undefined;

  return (
    <div className="sensitive-op-list-container">
      <div className="page-header">
        <h2>敏感操作管理</h2>
        <Space>
          <Button icon={<ReloadOutlined />} onClick={loadOperations} loading={loading}>
            刷新
          </Button>
          <Button type="primary" icon={<PlusOutlined />} onClick={handleCreate}>
            创建规则
          </Button>
        </Space>
      </div>

      <div className="sensitive-op-filter-bar">
        <Input
          placeholder="搜索操作名称或描述"
          prefix={<SearchOutlined />}
          value={searchText}
          onChange={(e) => setSearchText(e.target.value)}
          allowClear
          style={{ width: 260 }}
        />
        <Select
          placeholder="操作类型"
          value={typeFilter}
          onChange={setTypeFilter}
          allowClear
          style={{ width: 140 }}
        >
          {SENSITIVE_OP_TYPE_OPTIONS.map((opt) => (
            <Option key={opt.value} value={opt.value}>{opt.label}</Option>
          ))}
        </Select>
        <Select
          placeholder="风险等级"
          value={riskFilter}
          onChange={setRiskFilter}
          allowClear
          style={{ width: 120 }}
        >
          {RISK_LEVEL_OPTIONS.map((opt) => (
            <Option key={opt.value} value={opt.value}>
              <Tag color={opt.color} style={{ marginRight: 4 }}>{opt.label}</Tag>
            </Option>
          ))}
        </Select>
        <Select
          placeholder="状态"
          value={statusFilter}
          onChange={setStatusFilter}
          allowClear
          style={{ width: 110 }}
        >
          <Option value="enabled">已启用</Option>
          <Option value="disabled">已禁用</Option>
        </Select>
        {hasActiveFilters && (
          <Button type="link" onClick={clearFilters} size="small">
            清除筛选
          </Button>
        )}
        {operations.length > 0 && (
          <span className="sensitive-op-filter-summary">
            共 {operations.length} 条规则{filteredOps.length !== operations.length && `，显示 ${filteredOps.length} 条`}
          </span>
        )}
      </div>

      <Table
        columns={columns}
        dataSource={filteredOps}
        rowKey="id"
        loading={loading}
        locale={{ emptyText: emptyContent }}
        pagination={filteredOps.length > 10 ? {
          pageSize: 10,
          showSizeChanger: true,
          showTotal: (total) => `共 ${total} 条规则`,
        } : false}
        scroll={{ x: 1100 }}
        size="middle"
      />

      <SensitiveOpFormModal
        visible={modalVisible}
        mode={modalMode}
        operation={editingOp}
        onCancel={handleFormCancel}
        onSuccess={handleFormSuccess}
      />
    </div>
  );
};

function formatRelativeTime(dateStr: string): string {
  const date = new Date(dateStr);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMin = Math.floor(diffMs / 60000);
  const diffHour = Math.floor(diffMs / 3600000);
  const diffDay = Math.floor(diffMs / 86400000);

  if (diffMin < 1) return '刚刚';
  if (diffMin < 60) return `${diffMin} 分钟前`;
  if (diffHour < 24) return `${diffHour} 小时前`;
  if (diffDay < 7) return `${diffDay} 天前`;
  return date.toLocaleDateString('zh-CN', { year: 'numeric', month: '2-digit', day: '2-digit' });
}
