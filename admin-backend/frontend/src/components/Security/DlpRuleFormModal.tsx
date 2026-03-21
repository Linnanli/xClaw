import React, { useState, useEffect, useCallback } from 'react';
import { Modal, Form, Input, Select, message, Alert, Space, Button, Tag, Divider, Typography, Switch } from 'antd';
import { ExperimentOutlined, CheckCircleOutlined, CloseCircleOutlined, PlusOutlined } from '@ant-design/icons';
import { apiClient } from '../../api/client';
import type { DlpRule, CreateDlpRuleRequest, UpdateDlpRuleRequest, DlpRuleType, KeywordMatchMode, DlpDictionary } from '../../types';
import { DLP_SEVERITY_OPTIONS, DLP_CATEGORY_OPTIONS, DLP_RULE_TYPE_OPTIONS, KEYWORD_MATCH_MODE_OPTIONS } from '../../constants/dlp';

const { TextArea } = Input;
const { Option } = Select;
const { Text } = Typography;

export type DlpRuleFormMode = 'create' | 'edit';

interface DlpRuleFormModalProps {
  visible: boolean;
  mode: DlpRuleFormMode;
  rule?: DlpRule | null;
  onCancel: () => void;
  onSuccess: () => void;
}

/** 正则表达式语法验证 */
function validateRegex(pattern: string): { valid: boolean; error?: string } {
  if (!pattern) return { valid: false, error: '模式不能为空' };
  try {
    new RegExp(pattern);
    return { valid: true };
  } catch (e: any) {
    return { valid: false, error: e.message || '无效的正则表达式' };
  }
}

/** 规则测试结果 */
interface TestResult {
  matched: boolean;
  matches: string[];
  replaced: string;
}

/** 在前端本地测试正则匹配 */
function testPattern(pattern: string, testText: string, replacement: string): TestResult | null {
  const { valid } = validateRegex(pattern);
  if (!valid || !testText) return null;
  try {
    const regex = new RegExp(pattern, 'g');
    const matches = testText.match(regex) || [];
    const replaced = replacement ? testText.replace(regex, replacement) : testText;
    return { matched: matches.length > 0, matches, replaced };
  } catch {
    return null;
  }
}

/** 测试关键字匹配 */
function testKeywords(
  keywords: string[],
  testText: string,
  replacement: string,
  matchMode: KeywordMatchMode,
  caseSensitive: boolean,
): TestResult | null {
  if (!keywords.length || !testText) return null;
  const allMatches: string[] = [];
  let result = testText;

  for (const kw of keywords) {
    if (!kw) continue;
    const searchText = caseSensitive ? testText : testText.toLowerCase();
    const searchKw = caseSensitive ? kw : kw.toLowerCase();

    if (matchMode === 'exact' || matchMode === 'contains') {
      let idx = searchText.indexOf(searchKw);
      while (idx !== -1) {
        allMatches.push(testText.substring(idx, idx + kw.length));
        idx = searchText.indexOf(searchKw, idx + 1);
      }
    } else if (matchMode === 'whole_word') {
      const flags = caseSensitive ? 'g' : 'gi';
      const escaped = kw.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
      const regex = new RegExp(`\\b${escaped}\\b`, flags);
      const m = testText.match(regex);
      if (m) allMatches.push(...m);
    }
  }

  if (allMatches.length > 0) {
    for (const m of allMatches) {
      result = result.split(m).join(replacement || '***');
    }
  }

  return { matched: allMatches.length > 0, matches: allMatches, replaced: result };
}

export const DlpRuleFormModal: React.FC<DlpRuleFormModalProps> = ({
  visible,
  mode,
  rule,
  onCancel,
  onSuccess,
}) => {
  const [form] = Form.useForm();
  const [loading, setLoading] = useState(false);
  const [regexError, setRegexError] = useState<string | null>(null);
  const [testText, setTestText] = useState('');
  const [testResult, setTestResult] = useState<TestResult | null>(null);
  const [showTestArea, setShowTestArea] = useState(false);
  const [ruleType, setRuleType] = useState<DlpRuleType>('regex');
  const [keywords, setKeywords] = useState<string[]>([]);
  const [keywordInput, setKeywordInput] = useState('');
  const [dictionaries, setDictionaries] = useState<DlpDictionary[]>([]);
  const [loadingDicts, setLoadingDicts] = useState(false);

  const isEdit = mode === 'edit';
  const title = isEdit ? '编辑 DLP 规则' : '创建 DLP 规则';
  const okText = isEdit ? '保存' : '创建';

  // 编辑模式下填充表单
  useEffect(() => {
    if (visible && isEdit && rule) {
      const type = rule.rule_type || 'regex';
      setRuleType(type);
      form.setFieldsValue({
        name: rule.name,
        pattern: rule.pattern,
        replacement: rule.replacement || '',
        severity: rule.severity,
        category: rule.category,
        description: rule.description || '',
        rule_type: type,
        match_mode: (rule.rule_config as any)?.match_mode || 'contains',
        case_sensitive: (rule.rule_config as any)?.case_sensitive || false,
        dictionary_id: (rule.rule_config as any)?.dictionary_id || undefined,
      });
      setKeywords((rule.rule_config as any)?.keywords || []);
      setRegexError(null);
      setTestResult(null);
    } else if (visible && !isEdit) {
      form.resetFields();
      setRuleType('regex');
      setKeywords([]);
      setKeywordInput('');
      setRegexError(null);
      setTestResult(null);
      setTestText('');
      setShowTestArea(false);
    }
  }, [visible, mode, rule, form, isEdit]);

  // 加载字典列表
  useEffect(() => {
    if (visible && (ruleType === 'dictionary' || (isEdit && rule?.rule_type === 'dictionary'))) {
      setLoadingDicts(true);
      apiClient.get('/dlp-dictionaries')
        .then(res => setDictionaries(res.data.dictionaries || []))
        .catch(() => message.error('加载字典列表失败'))
        .finally(() => setLoadingDicts(false));
    }
  }, [visible, ruleType, isEdit, rule]);

  // 实时验证正则表达式
  const handlePatternChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const pattern = e.target.value;
    if (pattern && ruleType === 'regex') {
      const result = validateRegex(pattern);
      setRegexError(result.valid ? null : (result.error || '无效的正则表达式'));
    } else {
      setRegexError(null);
    }
    setTestResult(null);
  }, [ruleType]);

  // 添加关键字
  const handleAddKeyword = useCallback(() => {
    const trimmed = keywordInput.trim();
    if (!trimmed) return;
    if (keywords.includes(trimmed)) {
      message.warning('关键字已存在');
      return;
    }
    setKeywords(prev => [...prev, trimmed]);
    setKeywordInput('');
    setTestResult(null);
  }, [keywordInput, keywords]);

  // 删除关键字
  const handleRemoveKeyword = useCallback((kw: string) => {
    setKeywords(prev => prev.filter(k => k !== kw));
    setTestResult(null);
  }, []);

  // 规则类型切换
  const handleRuleTypeChange = useCallback((type: DlpRuleType) => {
    setRuleType(type);
    setRegexError(null);
    setTestResult(null);
    form.setFieldValue('rule_type', type);
    if (type === 'keyword') {
      form.setFieldValue('pattern', '');
    }
  }, [form]);

  // 执行测试
  const handleTest = useCallback(() => {
    const replacement = form.getFieldValue('replacement') || '***';
    if (!testText) {
      message.warning('请输入测试文本');
      return;
    }

    if (ruleType === 'regex') {
      const pattern = form.getFieldValue('pattern');
      if (!pattern) {
        message.warning('请先输入匹配模式');
        return;
      }
      const result = testPattern(pattern, testText, replacement);
      setTestResult(result);
    } else if (ruleType === 'keyword') {
      if (!keywords.length) {
        message.warning('请先添加关键字');
        return;
      }
      const matchMode = form.getFieldValue('match_mode') || 'contains';
      const caseSensitive = form.getFieldValue('case_sensitive') || false;
      const result = testKeywords(keywords, testText, replacement, matchMode, caseSensitive);
      setTestResult(result);
    } else if (ruleType === 'dictionary') {
      const dictId = form.getFieldValue('dictionary_id');
      const dict = dictionaries.find(d => d.id === dictId);
      if (!dict || !dict.keywords.length) {
        message.warning('请先选择包含关键字的字典');
        return;
      }
      const matchMode = form.getFieldValue('match_mode') || 'contains';
      const caseSensitive = form.getFieldValue('case_sensitive') || false;
      const result = testKeywords(dict.keywords, testText, replacement, matchMode, caseSensitive);
      setTestResult(result);
    }
  }, [form, testText, ruleType, keywords, dictionaries]);

  // 提交表单
  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();

      // 验证
      if (ruleType === 'regex') {
        const regexValidation = validateRegex(values.pattern);
        if (!regexValidation.valid) {
          message.error(`正则表达式无效: ${regexValidation.error}`);
          return;
        }
      } else if (ruleType === 'keyword') {
        if (!keywords.length) {
          message.error('请至少添加一个关键字');
          return;
        }
      } else if (ruleType === 'dictionary') {
        if (!values.dictionary_id) {
          message.error('请选择一个字典');
          return;
        }
      }

      setLoading(true);

      // 构建 pattern 和 rule_config
      let pattern: string;
      let ruleConfig: any = null;

      if (ruleType === 'keyword') {
        pattern = keywords.join(',');
        ruleConfig = {
          keywords,
          match_mode: values.match_mode || 'contains',
          case_sensitive: values.case_sensitive || false,
        };
      } else if (ruleType === 'dictionary') {
        const dict = dictionaries.find(d => d.id === values.dictionary_id);
        pattern = dict ? dict.keywords.join(',') : '';
        ruleConfig = {
          dictionary_id: values.dictionary_id,
          dictionary_name: dict?.name || '',
          match_mode: values.match_mode || 'contains',
          case_sensitive: values.case_sensitive || false,
        };
      } else {
        pattern = values.pattern;
      }

      if (isEdit && rule) {
        const request: UpdateDlpRuleRequest = {
          name: values.name,
          pattern,
          replacement: values.replacement || undefined,
          severity: values.severity,
          description: values.description || undefined,
          category: values.category,
          rule_type: ruleType,
          rule_config: ruleConfig,
        };
        await apiClient.put(`/dlp-rules/${rule.id}`, request);
        message.success('规则更新成功');
      } else {
        const request: CreateDlpRuleRequest = {
          name: values.name,
          pattern,
          replacement: values.replacement || undefined,
          severity: values.severity,
          description: values.description || undefined,
          category: values.category,
          rule_type: ruleType,
          rule_config: ruleConfig,
        };
        await apiClient.post('/dlp-rules', request);
        message.success('规则创建成功');
      }

      form.resetFields();
      setKeywords([]);
      setKeywordInput('');
      setTestText('');
      setTestResult(null);
      setShowTestArea(false);
      onSuccess();
    } catch (error: any) {
      if (error.response) {
        message.error(error.response.data?.error || `${isEdit ? '更新' : '创建'}规则失败`);
      }
    } finally {
      setLoading(false);
    }
  };

  const handleCancel = () => {
    form.resetFields();
    setRuleType('regex');
    setKeywords([]);
    setKeywordInput('');
    setRegexError(null);
    setTestText('');
    setTestResult(null);
    setShowTestArea(false);
    onCancel();
  };

  return (
    <Modal
      title={title}
      open={visible}
      onOk={handleSubmit}
      onCancel={handleCancel}
      confirmLoading={loading}
      okText={okText}
      cancelText="取消"
      width={680}
      destroyOnClose
    >
      <Form
        form={form}
        layout="vertical"
        initialValues={{
          severity: 'medium',
          category: 'pii',
          rule_type: 'regex',
          match_mode: 'contains',
          case_sensitive: false,
        }}
      >
        <Form.Item
          label="规则名"
          name="name"
          rules={[
            { required: true, message: '请输入规则名' },
            { min: 2, max: 50, message: '规则名长度为 2-50 个字符' },
          ]}
        >
          <Input placeholder="例如：身份证号检测" />
        </Form.Item>

        <Form.Item label="规则类型" name="rule_type">
          <Select onChange={handleRuleTypeChange} value={ruleType}>
            {DLP_RULE_TYPE_OPTIONS.map(opt => (
              <Option key={opt.value} value={opt.value}>
                {opt.label} — {opt.description}
              </Option>
            ))}
          </Select>
        </Form.Item>

        {ruleType === 'regex' ? (
          <Form.Item
            label="匹配模式（正则表达式）"
            name="pattern"
            rules={[
              { required: true, message: '请输入匹配模式' },
              { min: 1, max: 500, message: '匹配模式长度为 1-500 个字符' },
            ]}
            help={regexError ? (
              <Text type="danger">{regexError}</Text>
            ) : (
              <Text type="secondary">
                支持正则表达式，例如：\d{'{'}17{'}'}[\dXx] 匹配身份证号
              </Text>
            )}
            validateStatus={regexError ? 'error' : undefined}
          >
            <Input
              placeholder="例如：\d{17}[\dXx]"
              onChange={handlePatternChange}
              style={{ fontFamily: 'monospace' }}
            />
          </Form.Item>
        ) : ruleType === 'dictionary' ? (
          <>
            <Form.Item
              label="选择字典"
              name="dictionary_id"
              rules={[{ required: true, message: '请选择一个字典' }]}
            >
              <Select
                placeholder="请选择字典"
                loading={loadingDicts}
                showSearch
                optionFilterProp="label"
                options={dictionaries.map(d => ({
                  value: d.id,
                  label: `${d.name} (${d.keyword_count} 个关键字)`,
                }))}
              />
            </Form.Item>

            <Space style={{ width: '100%' }} size={16}>
              <Form.Item label="匹配模式" name="match_mode" style={{ width: 280 }}>
                <Select>
                  {KEYWORD_MATCH_MODE_OPTIONS.map(opt => (
                    <Option key={opt.value} value={opt.value}>
                      {opt.label} — {opt.description}
                    </Option>
                  ))}
                </Select>
              </Form.Item>

              <Form.Item label="区分大小写" name="case_sensitive" valuePropName="checked">
                <Switch />
              </Form.Item>
            </Space>
          </>
        ) : (
          <>
            <Form.Item label="关键字列表" required>
              <div style={{ marginBottom: 8 }}>
                {keywords.map(kw => (
                  <Tag
                    key={kw}
                    closable
                    onClose={() => handleRemoveKeyword(kw)}
                    style={{ marginBottom: 4 }}
                  >
                    {kw}
                  </Tag>
                ))}
                {keywords.length === 0 && (
                  <Text type="secondary">请添加至少一个关键字</Text>
                )}
              </div>
              <Space.Compact style={{ width: '100%' }}>
                <Input
                  placeholder="输入关键字后点击添加或按回车"
                  value={keywordInput}
                  onChange={e => setKeywordInput(e.target.value)}
                  onPressEnter={handleAddKeyword}
                  style={{ flex: 1 }}
                />
                <Button
                  type="primary"
                  icon={<PlusOutlined />}
                  onClick={handleAddKeyword}
                >
                  添加
                </Button>
              </Space.Compact>
            </Form.Item>

            <Space style={{ width: '100%' }} size={16}>
              <Form.Item label="匹配模式" name="match_mode" style={{ width: 280 }}>
                <Select>
                  {KEYWORD_MATCH_MODE_OPTIONS.map(opt => (
                    <Option key={opt.value} value={opt.value}>
                      {opt.label} — {opt.description}
                    </Option>
                  ))}
                </Select>
              </Form.Item>

              <Form.Item label="区分大小写" name="case_sensitive" valuePropName="checked">
                <Switch />
              </Form.Item>
            </Space>
          </>
        )}

        <Form.Item
          label="替换文本"
          name="replacement"
          rules={[{ max: 100, message: '替换文本长度不超过 100 个字符' }]}
          tooltip="匹配到的内容将被替换为此文本，留空则使用默认值 ***"
        >
          <Input placeholder="默认: ***" style={{ fontFamily: 'monospace' }} />
        </Form.Item>

        <Space style={{ width: '100%' }} size={16}>
          <Form.Item
            label="严重级别"
            name="severity"
            rules={[{ required: true, message: '请选择严重级别' }]}
            style={{ width: 300 }}
          >
            <Select placeholder="请选择严重级别">
              {DLP_SEVERITY_OPTIONS.map(option => (
                <Option key={option.value} value={option.value}>
                  <Tag color={option.color} style={{ marginRight: 4 }}>{option.label}</Tag>
                  {option.value === 'critical' && '— 阻断发送'}
                  {option.value === 'high' && '— 脱敏处理'}
                  {option.value === 'medium' && '— 脱敏处理'}
                  {option.value === 'low' && '— 脱敏处理'}
                </Option>
              ))}
            </Select>
          </Form.Item>

          <Form.Item
            label="分类"
            name="category"
            rules={[{ required: true, message: '请选择分类' }]}
            style={{ width: 300 }}
          >
            <Select placeholder="请选择分类">
              {DLP_CATEGORY_OPTIONS.map(option => (
                <Option key={option.value} value={option.value}>
                  {option.label}
                </Option>
              ))}
            </Select>
          </Form.Item>
        </Space>

        <Form.Item
          label="描述"
          name="description"
          rules={[{ max: 200, message: '描述长度不超过 200 个字符' }]}
        >
          <TextArea rows={2} placeholder="规则描述（可选）" />
        </Form.Item>
      </Form>

      {/* 规则测试区域 */}
      <Divider style={{ margin: '8px 0 16px' }} />
      {!showTestArea ? (
        <Button
          type="dashed"
          icon={<ExperimentOutlined />}
          onClick={() => setShowTestArea(true)}
          block
        >
          测试此规则
        </Button>
      ) : (
        <div className="dlp-test-area">
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 8 }}>
            <Text strong>规则测试</Text>
            <Button type="link" size="small" onClick={() => { setShowTestArea(false); setTestResult(null); setTestText(''); }}>
              收起
            </Button>
          </div>
          <TextArea
            rows={2}
            placeholder={ruleType === 'regex'
              ? '输入测试文本，例如：我的身份证号是330326199408015618'
              : '输入测试文本，例如：这是一段包含敏感关键字的内容'}
            value={testText}
            onChange={(e) => { setTestText(e.target.value); setTestResult(null); }}
            style={{ fontFamily: 'monospace', marginBottom: 8 }}
          />
          <Button
            type="primary"
            icon={<ExperimentOutlined />}
            onClick={handleTest}
            size="small"
            style={{ marginBottom: 8 }}
          >
            执行测试
          </Button>

          {testResult && (
            <Alert
              type={testResult.matched ? 'success' : 'warning'}
              showIcon
              icon={testResult.matched ? <CheckCircleOutlined /> : <CloseCircleOutlined />}
              message={testResult.matched ? `匹配成功（${testResult.matches.length} 处）` : '未匹配到内容'}
              description={testResult.matched ? (
                <div>
                  <div style={{ marginBottom: 4 }}>
                    <Text type="secondary">匹配内容：</Text>
                    {testResult.matches.map((m, i) => (
                      <Tag key={i} color="red" style={{ fontFamily: 'monospace' }}>{m}</Tag>
                    ))}
                  </div>
                  <div>
                    <Text type="secondary">替换结果：</Text>
                    <Text code>{testResult.replaced}</Text>
                  </div>
                </div>
              ) : undefined}
              style={{ marginTop: 4 }}
            />
          )}
        </div>
      )}
    </Modal>
  );
};

// 导出工具函数供测试使用
export { validateRegex, testPattern, testKeywords };
export type { TestResult };
