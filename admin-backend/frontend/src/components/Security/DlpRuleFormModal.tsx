import React, { useState, useEffect, useCallback } from 'react';
import { Modal, Form, Input, Select, message, Alert, Space, Button, Tag, Divider, Typography } from 'antd';
import { ExperimentOutlined, CheckCircleOutlined, CloseCircleOutlined } from '@ant-design/icons';
import { apiClient } from '../../api/client';
import type { DlpRule, CreateDlpRuleRequest, UpdateDlpRuleRequest } from '../../types';
import { DLP_SEVERITY_OPTIONS, DLP_CATEGORY_OPTIONS } from '../../constants/dlp';

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

  const isEdit = mode === 'edit';
  const title = isEdit ? '编辑 DLP 规则' : '创建 DLP 规则';
  const okText = isEdit ? '保存' : '创建';

  // 编辑模式下填充表单
  useEffect(() => {
    if (visible && isEdit && rule) {
      form.setFieldsValue({
        name: rule.name,
        pattern: rule.pattern,
        replacement: rule.replacement || '',
        severity: rule.severity,
        category: rule.category,
        description: rule.description || '',
      });
      setRegexError(null);
      setTestResult(null);
    } else if (visible && !isEdit) {
      form.resetFields();
      setRegexError(null);
      setTestResult(null);
      setTestText('');
      setShowTestArea(false);
    }
  }, [visible, mode, rule, form, isEdit]);

  // 实时验证正则表达式
  const handlePatternChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const pattern = e.target.value;
    if (pattern) {
      const result = validateRegex(pattern);
      setRegexError(result.valid ? null : (result.error || '无效的正则表达式'));
    } else {
      setRegexError(null);
    }
    // 清除之前的测试结果
    setTestResult(null);
  }, []);

  // 执行测试
  const handleTest = useCallback(() => {
    const pattern = form.getFieldValue('pattern');
    const replacement = form.getFieldValue('replacement') || '***';
    if (!pattern) {
      message.warning('请先输入匹配模式');
      return;
    }
    if (!testText) {
      message.warning('请输入测试文本');
      return;
    }
    const result = testPattern(pattern, testText, replacement);
    setTestResult(result);
  }, [form, testText]);

  // 提交表单
  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();

      // 提交前再次验证正则
      const regexValidation = validateRegex(values.pattern);
      if (!regexValidation.valid) {
        message.error(`正则表达式无效: ${regexValidation.error}`);
        return;
      }

      setLoading(true);

      if (isEdit && rule) {
        const request: UpdateDlpRuleRequest = {
          name: values.name,
          pattern: values.pattern,
          replacement: values.replacement || undefined,
          severity: values.severity,
          description: values.description || undefined,
          category: values.category,
        };
        await apiClient.put(`/dlp-rules/${rule.id}`, request);
        message.success('规则更新成功');
      } else {
        const request: CreateDlpRuleRequest = {
          name: values.name,
          pattern: values.pattern,
          replacement: values.replacement || undefined,
          severity: values.severity,
          description: values.description || undefined,
          category: values.category,
        };
        await apiClient.post('/dlp-rules', request);
        message.success('规则创建成功');
      }

      form.resetFields();
      setTestText('');
      setTestResult(null);
      setShowTestArea(false);
      onSuccess();
    } catch (error: any) {
      if (error.response) {
        message.error(error.response.data?.error || `${isEdit ? '更新' : '创建'}规则失败`);
      }
      // Form validation errors are handled by antd automatically
    } finally {
      setLoading(false);
    }
  };

  const handleCancel = () => {
    form.resetFields();
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
              支持正则表达式，例如：\d{'{'}17{'}'}[\dXx] 匹配身份证号，1[3-9]\d{'{'}9{'}'} 匹配手机号
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

        <Form.Item
          label="替换文本"
          name="replacement"
          rules={[
            { max: 100, message: '替换文本长度不超过 100 个字符' },
          ]}
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
          rules={[
            { max: 200, message: '描述长度不超过 200 个字符' },
          ]}
        >
          <TextArea
            rows={2}
            placeholder="规则描述（可选）"
          />
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
            placeholder="输入测试文本，例如：我的身份证号是330326199408015618"
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
export { validateRegex, testPattern };
export type { TestResult };
