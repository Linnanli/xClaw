import React, { useState, useEffect, useMemo, useCallback, useRef } from 'react';
import { Table, Button, Space, message, Popconfirm, Tag, Switch, Input, Select, Empty, Modal } from 'antd';
import {
  PlusOutlined,
  DeleteOutlined,
  EditOutlined,
  ReloadOutlined,
  SearchOutlined,
  SafetyCertificateOutlined,
  ExportOutlined,
  ImportOutlined,
  CheckCircleOutlined,
  StopOutlined,
} from '@ant-design/icons';
import type { ColumnsType } from 'antd/es/table';
import { DlpRuleFormModal, type DlpRuleFormMode } from '../../components/Security/DlpRuleFormModal';
import { apiClient } from '../../api/client';
import type { DlpRule } from '../../types';
import {
  getSeverityColor,
  getSeverityText,
  getCategoryText,
  DLP_SEVERITY_OPTIONS,
  DLP_CATEGORY_OPTIONS,
} from '../../constants/dlp';
import '../../styles/DlpRuleList.css';

const { Option } = Select;

export const DlpRuleList: React.FC = () => {
  const [rules, setRules] = useState<DlpRule[]>([]);
  const [loading, setLoading] = useState(false);
  const [modalVisible, setModalVisible] = useState(false);
  const [modalMode, setModalMode] = useState<DlpRuleFormMode>('create');
  const [editingRule, setEditingRule] = useState<DlpRule | null>(null);

  // 搜索和筛选状态
  const [searchText, setSearchText] = useState('');
  const [severityFilter, setSeverityFilter] = useState<string | undefined>(undefined);
  const [categoryFilter, setCategoryFilter] = useState<string | undefined>(undefined);
  const [statusFilter, setStatusFilter] = useState<string | undefined>(undefined);
  const [ruleTypeFilter, setRuleTypeFilter] = useState<string | undefined>(undefined);

  // 批量选择状态
  const [selectedRowKeys, setSelectedRowKeys] = useState<React.Key[]>([]);
  const fileInputRef = useRef<HTMLInputElement>(null);

  // 加载规则列表
  const loadRules = useCallback(async () => {
    setLoading(true);
    try {
      const response = await apiClient.get('/dlp-rules');
      setRules(response.data.rules || []);
    } catch (error: any) {
      message.error(error.response?.data?.error || '加载 DLP 规则失败');
    } finally {
      setLoading(false);
    }
  }, []);

  // 删除规则
  const handleDelete = useCallback(async (ruleId: string) => {
    try {
      await apiClient.delete(`/dlp-rules/${ruleId}`);
      message.success('删除规则成功');
      loadRules();
    } catch (error: any) {
      message.error(error.response?.data?.error || '删除规则失败');
    }
  }, [loadRules]);

  // 切换规则状态
  const handleToggleStatus = useCallback(async (ruleId: string, enabled: boolean) => {
    try {
      await apiClient.put(`/dlp-rules/${ruleId}`, { enabled });
      message.success(enabled ? '规则已启用' : '规则已禁用');
      loadRules();
    } catch (error: any) {
      message.error(error.response?.data?.error || '更新规则状态失败');
    }
  }, [loadRules]);

  // 打开创建弹窗
  const handleCreate = useCallback(() => {
    setModalMode('create');
    setEditingRule(null);
    setModalVisible(true);
  }, []);

  // 打开编辑弹窗
  const handleEdit = useCallback((rule: DlpRule) => {
    setModalMode('edit');
    setEditingRule(rule);
    setModalVisible(true);
  }, []);

  // 弹窗成功回调
  const handleFormSuccess = useCallback(() => {
    setModalVisible(false);
    setEditingRule(null);
    loadRules();
  }, [loadRules]);

  // 弹窗取消回调
  const handleFormCancel = useCallback(() => {
    setModalVisible(false);
    setEditingRule(null);
  }, []);

  // --- 批量操作 ---
  const handleBatchEnable = useCallback(async () => {
    if (selectedRowKeys.length === 0) return;
    try {
      await apiClient.post('/dlp-rules/batch/status', { ids: selectedRowKeys, enabled: true });
      message.success(`成功启用 ${selectedRowKeys.length} 条规则`);
      setSelectedRowKeys([]);
      loadRules();
    } catch (error: any) {
      message.error(error.response?.data?.error || '批量启用失败');
    }
  }, [selectedRowKeys, loadRules]);

  const handleBatchDisable = useCallback(async () => {
    if (selectedRowKeys.length === 0) return;
    try {
      await apiClient.post('/dlp-rules/batch/status', { ids: selectedRowKeys, enabled: false });
      message.success(`成功禁用 ${selectedRowKeys.length} 条规则`);
      setSelectedRowKeys([]);
      loadRules();
    } catch (error: any) {
      message.error(error.response?.data?.error || '批量禁用失败');
    }
  }, [selectedRowKeys, loadRules]);

  const handleBatchDelete = useCallback(async () => {
    if (selectedRowKeys.length === 0) return;
    try {
      await apiClient.post('/dlp-rules/batch/delete', { ids: selectedRowKeys });
      message.success(`成功删除 ${selectedRowKeys.length} 条规则`);
      setSelectedRowKeys([]);
      loadRules();
    } catch (error: any) {
      message.error(error.response?.data?.error || '批量删除失败');
    }
  }, [selectedRowKeys, loadRules]);

  // --- 导入导出 ---
  const handleExport = useCallback(async () => {
    try {
      const response = await apiClient.get('/dlp-rules/export');
      const data = JSON.stringify(response.data, null, 2);
      const blob = new Blob([data], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `dlp-rules-export-${new Date().toISOString().slice(0, 10)}.json`;
      a.click();
      URL.revokeObjectURL(url);
      message.success(`成功导出 ${response.data.count} 条规则`);
    } catch (error: any) {
      message.error(error.response?.data?.error || '导出失败');
    }
  }, []);

  const handleImportFile = useCallback(async (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;

    try {
      const text = await file.text();
      const data = JSON.parse(text);
      const rulesToImport = data.rules || data;

      if (!Array.isArray(rulesToImport) || rulesToImport.length === 0) {
        message.error('文件格式无效：未找到规则数据');
        return;
      }

      Modal.confirm({
        title: '确认导入',
        content: `将导入 ${rulesToImport.length} 条规则，是否继续？`,
        okText: '导入',
        cancelText: '取消',
        onOk: async () => {
          try {
            const response = await apiClient.post('/dlp-rules/import', { rules: rulesToImport });
            const { imported, errors } = response.data;
            if (errors && errors.length > 0) {
              message.warning(`导入 ${imported} 条成功，${errors.length} 条失败`);
            } else {
              message.success(`成功导入 ${imported} 条规则`);
            }
            loadRules();
          } catch (err: any) {
            message.error(err.response?.data?.error || '导入失败');
          }
        },
      });
    } catch {
      message.error('文件解析失败，请确保是有效的 JSON 文件');
    } finally {
      // 重置 input 以允许重复选择同一文件
      if (fileInputRef.current) fileInputRef.current.value = '';
    }
  }, [loadRules]);

  // 行选择配置
  const rowSelection = {
    selectedRowKeys,
    onChange: (keys: React.Key[]) => setSelectedRowKeys(keys),
  };

  useEffect(() => {
    loadRules();
  }, [loadRules]);

  // 前端筛选逻辑
  const filteredRules = useMemo(() => {
    return rules.filter((rule) => {
      // 文本搜索：匹配名称、模式、描述
      if (searchText) {
        const keyword = searchText.toLowerCase();
        const matchesSearch =
          rule.name.toLowerCase().includes(keyword) ||
          rule.pattern.toLowerCase().includes(keyword) ||
          (rule.description || '').toLowerCase().includes(keyword);
        if (!matchesSearch) return false;
      }
      // 严重级别筛选
      if (severityFilter && rule.severity !== severityFilter) return false;
      // 分类筛选
      if (categoryFilter && rule.category !== categoryFilter) return false;
      // 状态筛选
      if (statusFilter === 'enabled' && !rule.enabled) return false;
      if (statusFilter === 'disabled' && rule.enabled) return false;
      // 规则类型筛选
      if (ruleTypeFilter && (rule.rule_type || 'regex') !== ruleTypeFilter) return false;
      return true;
    });
  }, [rules, searchText, severityFilter, categoryFilter, statusFilter, ruleTypeFilter]);

  // 是否有活跃的筛选条件
  const hasActiveFilters = searchText || severityFilter || categoryFilter || statusFilter || ruleTypeFilter;

  // 清除所有筛选
  const clearFilters = useCallback(() => {
    setSearchText('');
    setSeverityFilter(undefined);
    setCategoryFilter(undefined);
    setStatusFilter(undefined);
    setRuleTypeFilter(undefined);
  }, []);

  // 表格列配置
  const columns: ColumnsType<DlpRule> = [
    {
      title: '规则名',
      dataIndex: 'name',
      key: 'name',
      width: 160,
      render: (name: string, record) => (
        <Button type="link" onClick={() => handleEdit(record)} style={{ padding: 0, fontWeight: 500 }}>
          {name}
        </Button>
      ),
    },
    {
      title: '类型',
      dataIndex: 'rule_type',
      key: 'rule_type',
      width: 90,
      render: (ruleType: string) => (
        <Tag color={ruleType === 'keyword' ? 'cyan' : ruleType === 'dictionary' ? 'green' : 'geekblue'}>
          {ruleType === 'keyword' ? '关键字' : ruleType === 'dictionary' ? '字典' : '正则'}
        </Tag>
      ),
    },
    {
      title: '匹配模式',
      dataIndex: 'pattern',
      key: 'pattern',
      width: 240,
      ellipsis: true,
      render: (pattern: string, record) => {
        if (record.rule_type === 'keyword' && record.rule_config && 'keywords' in record.rule_config) {
          const kws = (record.rule_config as any).keywords as string[];
          return (
            <span>
              {kws.slice(0, 3).map((kw, i) => (
                <Tag key={i} style={{ marginBottom: 2 }}>{kw}</Tag>
              ))}
              {kws.length > 3 && (
                <Tag>+{kws.length - 3}</Tag>
              )}
            </span>
          );
        }
        if (record.rule_type === 'dictionary' && record.rule_config) {
          const dictName = (record.rule_config as any).dictionary_name;
          return (
            <Tag color="green">{dictName || '字典'}</Tag>
          );
        }
        return <code className="dlp-pattern-cell">{pattern}</code>;
      },
    },
    {
      title: '替换文本',
      dataIndex: 'replacement',
      key: 'replacement',
      width: 100,
      render: (replacement: string) => (
        <code className="dlp-replacement-cell">{replacement || '***'}</code>
      ),
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
      width: 130,
      render: (category: string) => getCategoryText(category),
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
          <Button
            type="link"
            size="small"
            icon={<EditOutlined />}
            onClick={() => handleEdit(record)}
          >
            编辑
          </Button>
          <Popconfirm
            title="确认删除"
            description={`确定要删除规则「${record.name}」吗？删除后不可恢复。`}
            onConfirm={() => handleDelete(record.id)}
            okText="确定"
            cancelText="取消"
            okButtonProps={{ danger: true }}
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

  // 空状态组件
  const emptyContent = rules.length === 0 ? (
    <Empty
      image={<SafetyCertificateOutlined style={{ fontSize: 48, color: '#bfbfbf' }} />}
      description={
        <span>
          还没有 DLP 规则，<Button type="link" onClick={handleCreate} style={{ padding: 0 }}>创建第一条规则</Button> 来保护敏感数据
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
    <div className="dlp-rule-list-container">
      <div className="page-header">
        <h2>DLP 规则管理</h2>
        <Space>
          <Button
            icon={<ExportOutlined />}
            onClick={handleExport}
            disabled={rules.length === 0}
          >
            导出
          </Button>
          <Button
            icon={<ImportOutlined />}
            onClick={() => fileInputRef.current?.click()}
          >
            导入
          </Button>
          <input
            ref={fileInputRef}
            type="file"
            accept=".json"
            style={{ display: 'none' }}
            onChange={handleImportFile}
          />
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
            onClick={handleCreate}
          >
            创建规则
          </Button>
        </Space>
      </div>

      {/* 批量操作栏 */}
      {selectedRowKeys.length > 0 && (
        <div className="dlp-batch-bar">
          <span className="dlp-batch-info">已选择 {selectedRowKeys.length} 条规则</span>
          <Space>
            <Button size="small" icon={<CheckCircleOutlined />} onClick={handleBatchEnable}>
              批量启用
            </Button>
            <Button size="small" icon={<StopOutlined />} onClick={handleBatchDisable}>
              批量禁用
            </Button>
            <Popconfirm
              title="确认批量删除"
              description={`确定要删除选中的 ${selectedRowKeys.length} 条规则吗？`}
              onConfirm={handleBatchDelete}
              okText="确定"
              cancelText="取消"
              okButtonProps={{ danger: true }}
            >
              <Button size="small" danger icon={<DeleteOutlined />}>
                批量删除
              </Button>
            </Popconfirm>
            <Button size="small" type="link" onClick={() => setSelectedRowKeys([])}>
              取消选择
            </Button>
          </Space>
        </div>
      )}

      {/* 搜索和筛选栏 */}
      <div className="dlp-filter-bar">
        <Input
          placeholder="搜索规则名、匹配模式或描述"
          prefix={<SearchOutlined />}
          value={searchText}
          onChange={(e) => setSearchText(e.target.value)}
          allowClear
          style={{ width: 280 }}
        />
        <Select
          placeholder="严重级别"
          value={severityFilter}
          onChange={setSeverityFilter}
          allowClear
          style={{ width: 130 }}
        >
          {DLP_SEVERITY_OPTIONS.map(opt => (
            <Option key={opt.value} value={opt.value}>
              <Tag color={opt.color} style={{ marginRight: 4 }}>{opt.label}</Tag>
            </Option>
          ))}
        </Select>
        <Select
          placeholder="分类"
          value={categoryFilter}
          onChange={setCategoryFilter}
          allowClear
          style={{ width: 170 }}
        >
          {DLP_CATEGORY_OPTIONS.map(opt => (
            <Option key={opt.value} value={opt.value}>{opt.label}</Option>
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
        <Select
          placeholder="规则类型"
          value={ruleTypeFilter}
          onChange={setRuleTypeFilter}
          allowClear
          style={{ width: 130 }}
        >
          <Option value="regex">正则表达式</Option>
          <Option value="keyword">关键字</Option>
          <Option value="dictionary">字典</Option>
        </Select>
        {hasActiveFilters && (
          <Button type="link" onClick={clearFilters} size="small">
            清除筛选
          </Button>
        )}
        {rules.length > 0 && (
          <span className="dlp-filter-summary">
            共 {rules.length} 条规则{filteredRules.length !== rules.length && `，显示 ${filteredRules.length} 条`}
          </span>
        )}
      </div>

      <Table
        columns={columns}
        dataSource={filteredRules}
        rowKey="id"
        loading={loading}
        rowSelection={rowSelection}
        locale={{ emptyText: emptyContent }}
        pagination={filteredRules.length > 10 ? {
          pageSize: 10,
          showSizeChanger: true,
          showTotal: (total) => `共 ${total} 条规则`,
        } : false}
        scroll={{ x: 1200 }}
        size="middle"
      />

      <DlpRuleFormModal
        visible={modalVisible}
        mode={modalMode}
        rule={editingRule}
        onCancel={handleFormCancel}
        onSuccess={handleFormSuccess}
      />
    </div>
  );
};

/** 格式化为相对时间 */
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
