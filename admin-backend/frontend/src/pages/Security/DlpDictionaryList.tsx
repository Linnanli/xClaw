import React, { useState, useEffect, useMemo, useCallback, useRef } from 'react';
import { Table, Button, Space, message, Popconfirm, Tag, Input, Empty, Modal, Form, Tooltip } from 'antd';
import {
  PlusOutlined,
  DeleteOutlined,
  EditOutlined,
  ReloadOutlined,
  SearchOutlined,
  BookOutlined,
  UploadOutlined,
  CopyOutlined,
  WarningOutlined,
} from '@ant-design/icons';
import type { ColumnsType } from 'antd/es/table';
import { apiClient } from '../../api/client';
import type { DlpDictionary, CreateDictionaryRequest, UpdateDictionaryRequest } from '../../types';
import '../../styles/DlpDictionaryList.css';

const { TextArea } = Input;

/** 字典表单弹窗 */
interface DictFormModalProps {
  visible: boolean;
  mode: 'create' | 'edit';
  dictionary?: DlpDictionary | null;
  onCancel: () => void;
  onSuccess: () => void;
}

const DictFormModal: React.FC<DictFormModalProps> = ({ visible, mode, dictionary, onCancel, onSuccess }) => {
  const [form] = Form.useForm();
  const [submitting, setSubmitting] = useState(false);
  const [keywordInput, setKeywordInput] = useState('');
  const fileInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (visible && mode === 'edit' && dictionary) {
      form.setFieldsValue({
        name: dictionary.name,
        description: dictionary.description || '',
      });
      setKeywordInput(dictionary.keywords.join('\n'));
    } else if (visible && mode === 'create') {
      form.resetFields();
      setKeywordInput('');
    }
  }, [visible, mode, dictionary, form]);

  // 解析关键字列表
  const parseKeywords = (text: string): string[] => {
    return text
      .split('\n')
      .map((k) => k.trim())
      .filter((k) => k.length > 0);
  };

  // 去重统计
  const keywords = useMemo(() => parseKeywords(keywordInput), [keywordInput]);
  const uniqueKeywords = useMemo(() => [...new Set(keywords)], [keywords]);
  const duplicateCount = keywords.length - uniqueKeywords.length;

  // 去重操作
  const handleDedup = useCallback(() => {
    setKeywordInput(uniqueKeywords.join('\n'));
    message.success(`已去除 ${duplicateCount} 个重复关键字`);
  }, [uniqueKeywords, duplicateCount]);

  // 文件导入
  const handleFileImport = useCallback(async (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;

    try {
      const text = await file.text();
      const imported = parseKeywords(text);
      if (imported.length === 0) {
        message.error('文件中没有找到有效的关键字');
        return;
      }

      // 合并到现有关键字
      const existing = parseKeywords(keywordInput);
      const merged = [...new Set([...existing, ...imported])];
      setKeywordInput(merged.join('\n'));
      message.success(`从文件导入 ${imported.length} 个关键字（去重后共 ${merged.length} 个）`);
    } catch {
      message.error('文件读取失败');
    } finally {
      if (fileInputRef.current) fileInputRef.current.value = '';
    }
  }, [keywordInput]);

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();

      if (uniqueKeywords.length === 0) {
        message.error('请至少输入一个关键字');
        return;
      }

      setSubmitting(true);

      // 提交时自动去重
      if (mode === 'create') {
        const payload: CreateDictionaryRequest = {
          name: values.name,
          description: values.description || undefined,
          keywords: uniqueKeywords,
        };
        await apiClient.post('/dlp-dictionaries', payload);
        message.success('字典创建成功');
      } else if (dictionary) {
        const payload: UpdateDictionaryRequest = {
          name: values.name,
          description: values.description || undefined,
          keywords: uniqueKeywords,
        };
        await apiClient.put(`/dlp-dictionaries/${dictionary.id}`, payload);
        message.success('字典更新成功');
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
      title={mode === 'create' ? '创建字典' : '编辑字典'}
      open={visible}
      onCancel={onCancel}
      onOk={handleSubmit}
      confirmLoading={submitting}
      okText={mode === 'create' ? '创建' : '保存'}
      cancelText="取消"
      width={640}
      destroyOnClose
    >
      <Form form={form} layout="vertical">
        <Form.Item
          name="name"
          label="字典名称"
          rules={[{ required: true, message: '请输入字典名称' }]}
        >
          <Input placeholder="例如：敏感词字典" maxLength={100} />
        </Form.Item>
        <Form.Item name="description" label="描述">
          <Input placeholder="字典用途说明（可选）" maxLength={500} />
        </Form.Item>
        <Form.Item
          label={
            <Space>
              <span>关键字列表</span>
              <Tag color="blue">{uniqueKeywords.length} 个关键字</Tag>
              {duplicateCount > 0 && (
                <Tag color="warning" icon={<WarningOutlined />}>
                  {duplicateCount} 个重复
                </Tag>
              )}
            </Space>
          }
          required
        >
          <div className="dict-keyword-toolbar">
            <Button
              size="small"
              icon={<UploadOutlined />}
              onClick={() => fileInputRef.current?.click()}
            >
              从文件导入
            </Button>
            <input
              ref={fileInputRef}
              type="file"
              accept=".txt,.csv"
              style={{ display: 'none' }}
              onChange={handleFileImport}
            />
            {duplicateCount > 0 && (
              <Button size="small" type="link" onClick={handleDedup}>
                一键去重
              </Button>
            )}
          </div>
          <TextArea
            value={keywordInput}
            onChange={(e) => setKeywordInput(e.target.value)}
            placeholder="每行一个关键字，例如：&#10;机密&#10;绝密&#10;内部文件&#10;&#10;也可以点击「从文件导入」批量导入 .txt 或 .csv 文件"
            rows={12}
            style={{ fontFamily: 'monospace' }}
          />
          <div className="dict-keyword-hint">
            每行输入一个关键字，空行自动忽略，提交时自动去重
          </div>
        </Form.Item>
      </Form>
    </Modal>
  );
};

/** 字典管理主页面 */
export const DlpDictionaryList: React.FC = () => {
  const [dictionaries, setDictionaries] = useState<DlpDictionary[]>([]);
  const [loading, setLoading] = useState(false);
  const [searchText, setSearchText] = useState('');

  // 弹窗状态
  const [modalVisible, setModalVisible] = useState(false);
  const [modalMode, setModalMode] = useState<'create' | 'edit'>('create');
  const [editingDict, setEditingDict] = useState<DlpDictionary | null>(null);

  const loadDictionaries = useCallback(async () => {
    setLoading(true);
    try {
      const response = await apiClient.get('/dlp-dictionaries');
      setDictionaries(response.data.dictionaries || []);
    } catch (error: any) {
      message.error(error.response?.data?.error || '加载字典列表失败');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadDictionaries();
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  // 搜索过滤
  const filteredDicts = useMemo(() => {
    if (!searchText) return dictionaries;
    const kw = searchText.toLowerCase();
    return dictionaries.filter(
      (d) =>
        d.name.toLowerCase().includes(kw) ||
        (d.description || '').toLowerCase().includes(kw)
    );
  }, [dictionaries, searchText]);

  const hasActiveFilter = searchText.length > 0;

  // 打开创建弹窗
  const handleCreate = useCallback(() => {
    setEditingDict(null);
    setModalMode('create');
    setModalVisible(true);
  }, []);

  // 打开编辑弹窗
  const handleEdit = useCallback((dict: DlpDictionary) => {
    setEditingDict(dict);
    setModalMode('edit');
    setModalVisible(true);
  }, []);

  // 复制字典
  const handleCopy = useCallback(async (dict: DlpDictionary) => {
    try {
      const payload: CreateDictionaryRequest = {
        name: `${dict.name} (副本)`,
        description: dict.description,
        keywords: dict.keywords,
      };
      await apiClient.post('/dlp-dictionaries', payload);
      message.success('字典复制成功');
      loadDictionaries();
    } catch (error: any) {
      message.error(error.response?.data?.error || '复制失败');
    }
  }, [loadDictionaries]);

  // 删除字典
  const handleDelete = useCallback(async (id: string) => {
    try {
      await apiClient.delete(`/dlp-dictionaries/${id}`);
      message.success('字典已删除');
      loadDictionaries();
    } catch (error: any) {
      message.error(error.response?.data?.error || '删除失败');
    }
  }, [loadDictionaries]);

  // 弹窗成功回调
  const handleModalSuccess = useCallback(() => {
    setModalVisible(false);
    loadDictionaries();
  }, [loadDictionaries]);

  // 关键字预览渲染
  const renderKeywordPreview = (keywords: string[]) => {
    if (!keywords || keywords.length === 0) return <span style={{ color: '#bfbfbf' }}>无</span>;
    const maxShow = 5;
    const shown = keywords.slice(0, maxShow);
    const overflow = keywords.length - maxShow;
    return (
      <Space size={[4, 4]} wrap>
        {shown.map((kw, i) => (
          <Tag key={i}>{kw}</Tag>
        ))}
        {overflow > 0 && <Tag color="blue">+{overflow}</Tag>}
      </Space>
    );
  };

  const columns: ColumnsType<DlpDictionary> = [
    {
      title: '字典名称',
      dataIndex: 'name',
      key: 'name',
      width: 180,
      render: (name: string, record) => (
        <Button type="link" style={{ padding: 0, fontWeight: 500 }} onClick={() => handleEdit(record)}>
          {name}
        </Button>
      ),
    },
    {
      title: '描述',
      dataIndex: 'description',
      key: 'description',
      ellipsis: true,
      render: (desc: string | null) => desc || <span style={{ color: '#bfbfbf' }}>-</span>,
    },
    {
      title: '关键字数量',
      dataIndex: 'keyword_count',
      key: 'keyword_count',
      width: 120,
      sorter: (a, b) => a.keyword_count - b.keyword_count,
      render: (count: number) => <Tag color={count > 0 ? 'blue' : 'default'}>{count} 个</Tag>,
    },
    {
      title: '关键字预览',
      dataIndex: 'keywords',
      key: 'keywords',
      width: 300,
      render: (keywords: string[]) => renderKeywordPreview(keywords),
    },
    {
      title: '更新时间',
      dataIndex: 'updated_at',
      key: 'updated_at',
      width: 170,
      sorter: (a, b) => new Date(a.updated_at).getTime() - new Date(b.updated_at).getTime(),
      render: (date: string) =>
        new Date(date).toLocaleString('zh-CN', {
          year: 'numeric',
          month: '2-digit',
          day: '2-digit',
          hour: '2-digit',
          minute: '2-digit',
        }),
    },
    {
      title: '操作',
      key: 'actions',
      width: 200,
      render: (_, record) => (
        <Space size="small">
          <Button type="link" size="small" icon={<EditOutlined />} onClick={() => handleEdit(record)}>
            编辑
          </Button>
          <Tooltip title="复制为新字典">
            <Button type="link" size="small" icon={<CopyOutlined />} onClick={() => handleCopy(record)}>
              复制
            </Button>
          </Tooltip>
          <Popconfirm
            title="确认删除"
            description={`确定要删除字典「${record.name}」吗？`}
            onConfirm={() => handleDelete(record.id)}
            okText="删除"
            cancelText="取消"
            okButtonProps={{ danger: true }}
          >
            <Button type="link" size="small" danger icon={<DeleteOutlined />}>
              删除
            </Button>
          </Popconfirm>
        </Space>
      ),
    },
  ];

  const emptyContent = !loading && dictionaries.length === 0 ? (
    <Empty
      image={<BookOutlined style={{ fontSize: 48, color: '#bfbfbf' }} />}
      description={
        <span>
          还没有字典，<Button type="link" onClick={handleCreate} style={{ padding: 0 }}>创建第一个字典</Button>
        </span>
      }
    />
  ) : hasActiveFilter && filteredDicts.length === 0 ? (
    <Empty description="没有匹配的字典" />
  ) : undefined;

  return (
    <div className="dlp-dictionary-list-container">
      <div className="page-header">
        <h2>字典管理</h2>
        <Space>
          <Button icon={<ReloadOutlined />} onClick={loadDictionaries} loading={loading}>
            刷新
          </Button>
          <Button type="primary" icon={<PlusOutlined />} onClick={handleCreate}>
            创建字典
          </Button>
        </Space>
      </div>

      {/* 搜索栏 */}
      <div className="dict-filter-bar">
        <Input
          placeholder="搜索字典名称或描述"
          prefix={<SearchOutlined />}
          value={searchText}
          onChange={(e) => setSearchText(e.target.value)}
          allowClear
          style={{ width: 280 }}
        />
        <span className="dict-filter-summary">
          共 {dictionaries.length} 个字典
          {hasActiveFilter && filteredDicts.length !== dictionaries.length && `，显示 ${filteredDicts.length} 个`}
        </span>
      </div>

      <Table
        columns={columns}
        dataSource={filteredDicts}
        rowKey="id"
        loading={loading}
        locale={{ emptyText: emptyContent }}
        pagination={{
          pageSize: 20,
          showSizeChanger: true,
          pageSizeOptions: ['10', '20', '50'],
          showTotal: (t) => `共 ${t} 个字典`,
        }}
        scroll={{ x: 900 }}
        size="middle"
      />

      <DictFormModal
        visible={modalVisible}
        mode={modalMode}
        dictionary={editingDict}
        onCancel={() => setModalVisible(false)}
        onSuccess={handleModalSuccess}
      />
    </div>
  );
};
