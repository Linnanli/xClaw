import React, { useState, useEffect, useMemo, useCallback } from 'react';
import { Table, Button, Space, message, Popconfirm, Tag, Input, Empty, Modal, Form } from 'antd';
import {
  PlusOutlined,
  DeleteOutlined,
  EditOutlined,
  ReloadOutlined,
  SearchOutlined,
  BookOutlined,
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

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();
      const keywords = keywordInput
        .split('\n')
        .map((k: string) => k.trim())
        .filter((k: string) => k.length > 0);

      if (keywords.length === 0) {
        message.error('请至少输入一个关键字');
        return;
      }

      setSubmitting(true);

      if (mode === 'create') {
        const payload: CreateDictionaryRequest = {
          name: values.name,
          description: values.description || undefined,
          keywords,
        };
        await apiClient.post('/dlp-dictionaries', payload);
        message.success('字典创建成功');
      } else if (dictionary) {
        const payload: UpdateDictionaryRequest = {
          name: values.name,
          description: values.description || undefined,
          keywords,
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

  const keywordCount = keywordInput
    .split('\n')
    .map((k: string) => k.trim())
    .filter((k: string) => k.length > 0).length;

  return (
    <Modal
      title={mode === 'create' ? '创建字典' : '编辑字典'}
      open={visible}
      onCancel={onCancel}
      onOk={handleSubmit}
      confirmLoading={submitting}
      okText={mode === 'create' ? '创建' : '保存'}
      cancelText="取消"
      width={600}
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
            <span>
              关键字列表
              <Tag style={{ marginLeft: 8 }}>{keywordCount} 个关键字</Tag>
            </span>
          }
          required
        >
          <TextArea
            value={keywordInput}
            onChange={(e) => setKeywordInput(e.target.value)}
            placeholder="每行一个关键字，例如：&#10;机密&#10;绝密&#10;内部文件"
            rows={10}
            style={{ fontFamily: 'monospace' }}
          />
          <div className="dict-keyword-hint">每行输入一个关键字，空行会被自动忽略</div>
        </Form.Item>
      </Form>
    </Modal>
  );
};


/** 字典管理列表页 */
export const DlpDictionaryList: React.FC = () => {
  const [dictionaries, setDictionaries] = useState<DlpDictionary[]>([]);
  const [loading, setLoading] = useState(false);
  const [modalVisible, setModalVisible] = useState(false);
  const [modalMode, setModalMode] = useState<'create' | 'edit'>('create');
  const [editingDict, setEditingDict] = useState<DlpDictionary | null>(null);
  const [searchText, setSearchText] = useState('');

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
  }, [loadDictionaries]);

  const handleDelete = useCallback(async (id: string) => {
    try {
      await apiClient.delete(`/dlp-dictionaries/${id}`);
      message.success('字典删除成功');
      loadDictionaries();
    } catch (error: any) {
      message.error(error.response?.data?.error || '删除字典失败');
    }
  }, [loadDictionaries]);

  const handleCreate = useCallback(() => {
    setModalMode('create');
    setEditingDict(null);
    setModalVisible(true);
  }, []);

  const handleEdit = useCallback((dict: DlpDictionary) => {
    setModalMode('edit');
    setEditingDict(dict);
    setModalVisible(true);
  }, []);

  const handleFormSuccess = useCallback(() => {
    setModalVisible(false);
    setEditingDict(null);
    loadDictionaries();
  }, [loadDictionaries]);

  const handleFormCancel = useCallback(() => {
    setModalVisible(false);
    setEditingDict(null);
  }, []);

  const filteredDictionaries = useMemo(() => {
    if (!searchText) return dictionaries;
    const keyword = searchText.toLowerCase();
    return dictionaries.filter(
      (d) =>
        d.name.toLowerCase().includes(keyword) ||
        (d.description || '').toLowerCase().includes(keyword)
    );
  }, [dictionaries, searchText]);

  const columns: ColumnsType<DlpDictionary> = [
    {
      title: '字典名称',
      dataIndex: 'name',
      key: 'name',
      width: 200,
      render: (name: string, record) => (
        <Button type="link" onClick={() => handleEdit(record)} style={{ padding: 0, fontWeight: 500 }}>
          {name}
        </Button>
      ),
    },
    {
      title: '描述',
      dataIndex: 'description',
      key: 'description',
      width: 250,
      ellipsis: true,
      render: (desc: string) => desc || <span style={{ color: '#bfbfbf' }}>-</span>,
    },
    {
      title: '关键字数量',
      dataIndex: 'keyword_count',
      key: 'keyword_count',
      width: 120,
      render: (count: number) => <Tag color="blue">{count} 个</Tag>,
    },
    {
      title: '关键字预览',
      dataIndex: 'keywords',
      key: 'keywords',
      width: 300,
      render: (keywords: string[]) => {
        if (!keywords || keywords.length === 0) return <span style={{ color: '#bfbfbf' }}>无</span>;
        return (
          <span>
            {keywords.slice(0, 5).map((kw, i) => (
              <Tag key={i} style={{ marginBottom: 2 }}>{kw}</Tag>
            ))}
            {keywords.length > 5 && <Tag>+{keywords.length - 5}</Tag>}
          </span>
        );
      },
    },
    {
      title: '更新时间',
      dataIndex: 'updated_at',
      key: 'updated_at',
      width: 160,
      render: (date: string) => {
        const d = new Date(date);
        return d.toLocaleDateString('zh-CN', { year: 'numeric', month: '2-digit', day: '2-digit' });
      },
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
            description={`确定要删除字典「${record.name}」吗？使用此字典的规则将受到影响。`}
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

  const emptyContent = dictionaries.length === 0 ? (
    <Empty
      image={<BookOutlined style={{ fontSize: 48, color: '#bfbfbf' }} />}
      description={
        <span>
          还没有字典，<Button type="link" onClick={handleCreate} style={{ padding: 0 }}>创建第一个字典</Button> 来管理关键字集合
        </span>
      }
    />
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

      <div className="dlp-filter-bar">
        <Input
          placeholder="搜索字典名称或描述"
          prefix={<SearchOutlined />}
          value={searchText}
          onChange={(e) => setSearchText(e.target.value)}
          allowClear
          style={{ width: 300 }}
        />
        {dictionaries.length > 0 && (
          <span className="dlp-filter-summary">
            共 {dictionaries.length} 个字典
            {filteredDictionaries.length !== dictionaries.length && `，显示 ${filteredDictionaries.length} 个`}
          </span>
        )}
      </div>

      <Table
        columns={columns}
        dataSource={filteredDictionaries}
        rowKey="id"
        loading={loading}
        locale={{ emptyText: emptyContent }}
        pagination={filteredDictionaries.length > 10 ? {
          pageSize: 10,
          showSizeChanger: true,
          showTotal: (total) => `共 ${total} 个字典`,
        } : false}
        scroll={{ x: 1100 }}
        size="middle"
      />

      <DictFormModal
        visible={modalVisible}
        mode={modalMode}
        dictionary={editingDict}
        onCancel={handleFormCancel}
        onSuccess={handleFormSuccess}
      />
    </div>
  );
};
