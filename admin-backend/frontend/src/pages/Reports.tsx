import React, { useState, useEffect, useCallback } from 'react';
import { Button, Card, Row, Col, Statistic, Spin, message, Empty } from 'antd';
import {
  ReloadOutlined,
  SafetyCertificateOutlined,
  WarningOutlined,
  DesktopOutlined,
  FileProtectOutlined,
  RiseOutlined,
  BarChartOutlined,
} from '@ant-design/icons';
import { apiClient } from '../api/client';
import '../styles/Reports.css';

interface ReportData {
  dlp_rules: { total: number; enabled: number; disabled: number; by_severity: Array<{ severity: string; count: number }> };
  sensitive_ops: { total: number; enabled: number; disabled: number; by_risk: Array<{ risk_level: string; count: number }> };
  dictionaries: { total: number; total_keywords: number };
  clients: { total: number; online: number; offline: number };
  policy_changes: { total: number; recent_7d: number; by_type: Array<{ type: string; count: number }> };
  audit_logs: { total: number; recent_7d: number };
}

const SEVERITY_COLORS: Record<string, string> = {
  critical: '#cf1322', high: '#fa541c', medium: '#faad14', low: '#52c41a',
};

const RISK_COLORS: Record<string, string> = {
  critical: '#cf1322', high: '#fa541c', medium: '#faad14', low: '#52c41a',
};

const SEVERITY_LABELS: Record<string, string> = {
  critical: '严重', high: '高', medium: '中', low: '低',
};

const RISK_LABELS: Record<string, string> = {
  critical: '严重', high: '高', medium: '中', low: '低',
};

const CHANGE_TYPE_LABELS: Record<string, string> = {
  create: '创建', update: '修改', delete: '删除', enable: '启用', disable: '禁用', import: '导入',
};

/** 分布条形图组件 */
const DistributionBar: React.FC<{ items: Array<{ label: string; count: number; color: string }>; total: number }> = ({ items, total }) => {
  if (total === 0) return <Empty description="暂无数据" image={Empty.PRESENTED_IMAGE_SIMPLE} />;
  return (
    <div>
      {items.map((item, i) => (
        <div key={i} className="reports-distribution-item">
          <span className="reports-distribution-label">{item.label}</span>
          <div className="reports-distribution-bar">
            <div
              className="reports-distribution-bar-fill"
              style={{ width: `${Math.max((item.count / total) * 100, 8)}%`, background: item.color }}
            >
              {item.count}
            </div>
          </div>
        </div>
      ))}
    </div>
  );
};

export const Reports: React.FC = () => {
  const [data, setData] = useState<ReportData | null>(null);
  const [loading, setLoading] = useState(false);

  const loadReport = useCallback(async () => {
    setLoading(true);
    try {
      // 并行加载各模块数据
      const [rulesRes, opsRes, dictsRes, clientsRes, changesRes, logsRes] = await Promise.allSettled([
        apiClient.get('/dlp-rules'),
        apiClient.get('/sensitive-operations'),
        apiClient.get('/dlp-dictionaries'),
        apiClient.get('/clients/stats'),
        apiClient.get('/policy-changes/stats'),
        apiClient.get('/audit-logs', { params: { limit: 1 } }),
      ]);

      const rules = rulesRes.status === 'fulfilled' ? rulesRes.value.data.rules || [] : [];
      const ops = opsRes.status === 'fulfilled' ? opsRes.value.data.operations || [] : [];
      const dicts = dictsRes.status === 'fulfilled' ? dictsRes.value.data.dictionaries || [] : [];
      const clientStats = clientsRes.status === 'fulfilled' ? clientsRes.value.data : { total: 0, online: 0, offline: 0 };
      const changeStats = changesRes.status === 'fulfilled' ? changesRes.value.data : { total: 0, trend_7d: [], by_change_type: [] };
      const logTotal = logsRes.status === 'fulfilled' ? (logsRes.value.data.total || logsRes.value.data.logs?.length || 0) : 0;

      // 统计 DLP 规则
      const enabledRules = rules.filter((r: any) => r.enabled);
      const bySeverity: Record<string, number> = {};
      rules.forEach((r: any) => { bySeverity[r.severity] = (bySeverity[r.severity] || 0) + 1; });

      // 统计敏感操作
      const enabledOps = ops.filter((o: any) => o.enabled);
      const byRisk: Record<string, number> = {};
      ops.forEach((o: any) => { byRisk[o.risk_level] = (byRisk[o.risk_level] || 0) + 1; });

      // 统计字典
      const totalKeywords = dicts.reduce((sum: number, d: any) => sum + (d.keyword_count || 0), 0);

      // 最近 7 天变更
      const recent7d = (changeStats.trend_7d || []).reduce((sum: number, t: any) => sum + t.count, 0);

      setData({
        dlp_rules: {
          total: rules.length,
          enabled: enabledRules.length,
          disabled: rules.length - enabledRules.length,
          by_severity: Object.entries(bySeverity).map(([severity, count]) => ({ severity, count: count as number })),
        },
        sensitive_ops: {
          total: ops.length,
          enabled: enabledOps.length,
          disabled: ops.length - enabledOps.length,
          by_risk: Object.entries(byRisk).map(([risk_level, count]) => ({ risk_level, count: count as number })),
        },
        dictionaries: { total: dicts.length, total_keywords: totalKeywords },
        clients: clientStats,
        policy_changes: {
          total: changeStats.total || 0,
          recent_7d: recent7d,
          by_type: changeStats.by_change_type || [],
        },
        audit_logs: { total: logTotal, recent_7d: 0 },
      });
    } catch (error: any) {
      message.error('加载报表数据失败');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadReport();
  }, [loadReport]);

  return (
    <div className="reports-container">
      <div className="page-header">
        <h2>统计报表</h2>
        <Button icon={<ReloadOutlined />} onClick={loadReport} loading={loading}>
          刷新
        </Button>
      </div>

      <Spin spinning={loading}>
        {data ? (
          <>
            {/* 概览统计 */}
            <Row gutter={16} className="reports-stats-row">
              <Col span={4}>
                <Card size="small">
                  <Statistic title="DLP 规则" value={data.dlp_rules.total} prefix={<SafetyCertificateOutlined />} />
                </Card>
              </Col>
              <Col span={4}>
                <Card size="small">
                  <Statistic title="敏感操作" value={data.sensitive_ops.total} prefix={<WarningOutlined />} />
                </Card>
              </Col>
              <Col span={4}>
                <Card size="small">
                  <Statistic title="字典" value={data.dictionaries.total} prefix={<FileProtectOutlined />} suffix={`/ ${data.dictionaries.total_keywords} 词`} />
                </Card>
              </Col>
              <Col span={4}>
                <Card size="small">
                  <Statistic title="客户端" value={data.clients.total} prefix={<DesktopOutlined />} suffix={`(${data.clients.online} 在线)`} />
                </Card>
              </Col>
              <Col span={4}>
                <Card size="small">
                  <Statistic title="策略变更" value={data.policy_changes.total} prefix={<RiseOutlined />} />
                </Card>
              </Col>
              <Col span={4}>
                <Card size="small">
                  <Statistic title="近 7 天变更" value={data.policy_changes.recent_7d} prefix={<BarChartOutlined />} />
                </Card>
              </Col>
            </Row>

            {/* 详细分布 */}
            <Row gutter={16}>
              <Col span={12}>
                <Card title="DLP 规则严重级别分布" size="small" className="reports-section">
                  <DistributionBar
                    items={data.dlp_rules.by_severity.map((s) => ({
                      label: SEVERITY_LABELS[s.severity] || s.severity,
                      count: s.count,
                      color: SEVERITY_COLORS[s.severity] || '#1890ff',
                    }))}
                    total={data.dlp_rules.total}
                  />
                  <div style={{ marginTop: 12, fontSize: 13, color: '#999' }}>
                    已启用 {data.dlp_rules.enabled} / 已禁用 {data.dlp_rules.disabled}
                  </div>
                </Card>
              </Col>
              <Col span={12}>
                <Card title="敏感操作风险等级分布" size="small" className="reports-section">
                  <DistributionBar
                    items={data.sensitive_ops.by_risk.map((r) => ({
                      label: RISK_LABELS[r.risk_level] || r.risk_level,
                      count: r.count,
                      color: RISK_COLORS[r.risk_level] || '#1890ff',
                    }))}
                    total={data.sensitive_ops.total}
                  />
                  <div style={{ marginTop: 12, fontSize: 13, color: '#999' }}>
                    已启用 {data.sensitive_ops.enabled} / 已禁用 {data.sensitive_ops.disabled}
                  </div>
                </Card>
              </Col>
            </Row>

            {/* 策略变更类型分布 */}
            {data.policy_changes.by_type.length > 0 && (
              <Card title="策略变更类型分布" size="small" className="reports-section" style={{ marginTop: 16 }}>
                <Row gutter={16}>
                  {data.policy_changes.by_type.map((item) => (
                    <Col span={4} key={item.type}>
                      <Statistic
                        title={CHANGE_TYPE_LABELS[item.type] || item.type}
                        value={item.count}
                      />
                    </Col>
                  ))}
                </Row>
              </Card>
            )}
          </>
        ) : !loading ? (
          <Empty description="暂无报表数据" />
        ) : null}
      </Spin>
    </div>
  );
};
