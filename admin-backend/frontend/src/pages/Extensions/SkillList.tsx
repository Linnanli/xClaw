import React, { useState, useEffect, useCallback } from 'react';
import { Table, Button, Tag, Input, Empty, Card, Row, Col, Statistic, Switch, message } from 'antd';
import {
  ReloadOutlined,
  SearchOutlined,
  ThunderboltOutlined,
  AppstoreOutlined,
} from '@ant-design/icons';
import type { ColumnsType } from 'antd/es/table';
import { apiClient } from '../../api/client';

interface Skill {
  id: string;
  name: string;
  description?: string;
  version: string;
  author?: string;
  enabled: boolean;
  created_at: string;
  updated_at: string;
}

export const SkillList: React.FC = () => {
  const [skills, setSkills] = useState<Skill[]>([]);
  const [loading, setLoading] = useState(false);
  const [searchText, setSearchText] = useState('');

  const loadSkills = useCallback(async () => {
    setLoading(true);
    try {
      // 尝试从主项目 API 获取技能列表
      const response = await apiClient.get('/skills');
      setSkills(Array.isArray(response.data.skills) ? response.data.skills : Array.isArray(response.data) ? response.data : []);
    } catch {
      // API 可能尚未实现，显示空状态
      setSkills([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadSkills();
  }, [loadSkills]);

  const filteredSkills = skills.filter((s) => {
    if (!searchText) return true;
    const kw = searchText.toLowerCase();
    return s.name.toLowerCase().includes(kw) || (s.description || '').toLowerCase().includes(kw);
  });

  const handleToggle = useCallback(async (skill: Skill) => {
    const action = skill.enabled ? 'disable' : 'enable';
    try {
      await apiClient.post(`/skills/${skill.id}/${action}`);
      message.success(`${skill.name} 已${skill.enabled ? '禁用' : '启用'}`);
      loadSkills();
    } catch {
      message.error(`操作失败`);
    }
  }, [loadSkills]);

  const columns: ColumnsType<Skill> = [
    {
      title: '技能名称',
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
      render: (v: string) => <Tag color="blue">{v}</Tag>,
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
      render: (enabled: boolean, record: Skill) => (
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
        <h2 style={{ margin: 0, fontSize: 20, fontWeight: 600 }}>技能管理</h2>
        <Button icon={<ReloadOutlined />} onClick={loadSkills} loading={loading}>刷新</Button>
      </div>

      <Row gutter={16} style={{ marginBottom: 16 }}>
        <Col span={8}>
          <Card size="small">
            <Statistic title="已安装技能" value={skills.length} prefix={<ThunderboltOutlined />} />
          </Card>
        </Col>
      </Row>

      <div style={{ display: 'flex', gap: 12, marginBottom: 16 }}>
        <Input
          placeholder="搜索技能名称或描述"
          prefix={<SearchOutlined />}
          value={searchText}
          onChange={(e) => setSearchText(e.target.value)}
          allowClear
          style={{ width: 280 }}
        />
        <span style={{ color: '#999', fontSize: 13, marginLeft: 'auto', lineHeight: '32px' }}>
          共 {skills.length} 个技能
        </span>
      </div>

      <Table
        columns={columns}
        dataSource={filteredSkills}
        rowKey="id"
        loading={loading}
        locale={{
          emptyText: (
            <Empty
              image={<AppstoreOutlined style={{ fontSize: 48, color: '#bfbfbf' }} />}
              description="暂无已安装的技能，技能将通过主项目自动同步"
            />
          ),
        }}
        pagination={filteredSkills.length > 10 ? { pageSize: 10, showSizeChanger: true, showTotal: (t) => `共 ${t} 个技能` } : false}
        size="middle"
      />
    </div>
  );
};
