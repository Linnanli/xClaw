# DLP 多维度规则系统设计方案

## 概述

本文档定义了一个完整的多维度 DLP（数据泄露防护）规则系统，支持多种检测方法和匹配模式。

## 规则类型分类

### 1. 基于模式的检测（Pattern-Based Detection）

#### 1.1 正则表达式（Regex）
**用途**：检测结构化的敏感数据

**示例**：
- 身份证号：`\d{17}[\dXx]`
- 手机号：`1[3-9]\d{9}`
- 信用卡号：`\d{4}[\s-]?\d{4}[\s-]?\d{4}[\s-]?\d{4}`
- API Key：`[A-Za-z0-9]{32,}`

**配置结构**：
```json
{
  "type": "regex",
  "pattern": "\\d{17}[\\dXx]",
  "flags": "i",
  "description": "中国身份证号"
}
```

#### 1.2 关键字匹配（Keyword）
**用途**：检测特定的敏感词汇

**匹配模式**：
- 精确匹配（Exact）
- 前缀匹配（Prefix）
- 后缀匹配（Suffix）
- 包含匹配（Contains）
- 全词匹配（Whole Word）

**示例**：
- 精确匹配：`"机密"`
- 前缀匹配：`"内部-*"`
- 全词匹配：`"password"` (不匹配 "passwords")

**配置结构**：
```json
{
  "type": "keyword",
  "keywords": ["机密", "绝密", "内部"],
  "match_mode": "exact",
  "case_sensitive": false
}
```

#### 1.3 字典匹配（Dictionary）
**用途**：检测大量预定义的敏感词

**特点**：
- 支持大规模词表（10万+）
- 使用 Aho-Corasick 算法快速匹配
- 支持词表分类（政治、色情、暴力等）

**示例**：
- 敏感人名字典
- 敏感地名字典
- 违禁词字典

**配置结构**：
```json
{
  "type": "dictionary",
  "dictionary_id": "sensitive_names",
  "dictionary_source": "file://dictionaries/names.txt",
  "match_mode": "whole_word"
}
```

### 2. 基于指纹的检测（Fingerprint-Based Detection）

#### 2.1 数据库指纹（Database Fingerprint）
**用途**：检测数据库导出的敏感数据

**检测方法**：
- SQL 语句特征
- 表结构特征
- 数据格式特征（CSV、JSON、XML）
- 列名特征（user_id, password, email）

**示例**：
```sql
-- 检测 SQL 导出
INSERT INTO users (id, name, email, password) VALUES ...
SELECT * FROM customers WHERE ...
```

**配置结构**：
```json
{
  "type": "database_fingerprint",
  "detect_sql_statements": true,
  "detect_table_structures": true,
  "sensitive_column_names": ["password", "ssn", "credit_card"],
  "min_rows": 10
}
```

#### 2.2 文档指纹（Document Fingerprint）
**用途**：检测特定文档的泄露

**检测方法**：
- 文档哈希（MD5、SHA256）
- 部分内容哈希（Fuzzy Hash）
- 文档元数据（作者、标题、创建时间）
- 文档结构特征

**示例**：
- 检测公司财报文档
- 检测合同模板
- 检测源代码文件

**配置结构**：
```json
{
  "type": "document_fingerprint",
  "fingerprint_method": "fuzzy_hash",
  "document_hashes": [
    "d41d8cd98f00b204e9800998ecf8427e",
    "098f6bcd4621d373cade4e832627b4f6"
  ],
  "similarity_threshold": 0.8
}
```

### 3. 基于内容的检测（Content-Based Detection）

#### 3.1 图像识别（Image Recognition）
**用途**：检测图像中的敏感信息

**检测方法**：
- OCR 文字识别（身份证、护照）
- 人脸识别
- 物体识别（枪支、毒品）
- 场景识别（办公室、机房）

**示例**：
- 检测身份证照片
- 检测屏幕截图中的敏感信息
- 检测不当图片

**配置结构**：
```json
{
  "type": "image_recognition",
  "detection_methods": ["ocr", "face_detection"],
  "ocr_languages": ["zh", "en"],
  "min_confidence": 0.8
}
```

#### 3.2 语义模型（Semantic Model）
**用途**：基于语义理解检测敏感内容

**检测方法**：
- 文本分类（敏感话题）
- 情感分析（负面内容）
- 实体识别（人名、地名、组织）
- 意图识别（泄密意图）

**示例**：
- 检测讨论公司机密的对话
- 检测泄露客户信息的意图
- 检测不当言论

**配置结构**：
```json
{
  "type": "semantic_model",
  "model_type": "text_classification",
  "model_path": "models/sensitive_topic_classifier.onnx",
  "categories": ["company_secret", "customer_data", "financial_info"],
  "min_confidence": 0.7
}
```

### 4. 基于文件的检测（File-Based Detection）

#### 4.1 文件类型（File Type）
**用途**：检测特定类型的文件

**检测方法**：
- 文件扩展名
- MIME 类型
- 文件魔数（Magic Number）
- 文件头特征

**示例**：
- 禁止上传 .exe 文件
- 禁止发送 .sql 文件
- 禁止传输 .zip 加密文件

**配置结构**：
```json
{
  "type": "file_type",
  "allowed_types": ["image/jpeg", "image/png", "application/pdf"],
  "blocked_types": ["application/x-executable", "application/x-sql"],
  "check_magic_number": true
}
```

#### 4.2 文件标签（File Label）
**用途**：基于文件标签检测敏感文件

**检测方法**：
- 文件系统标签（macOS Tags、Windows Labels）
- 自定义元数据标签
- 文档分类标签

**示例**：
- 检测标记为"机密"的文件
- 检测标记为"内部"的文档
- 检测标记为"客户数据"的表格

**配置结构**：
```json
{
  "type": "file_label",
  "sensitive_labels": ["机密", "绝密", "内部"],
  "label_sources": ["filesystem", "metadata", "custom"]
}
```

## 规则组合和优先级

### 组合规则（Composite Rules）

支持多个规则的逻辑组合：

```json
{
  "type": "composite",
  "operator": "AND",
  "rules": [
    {
      "type": "keyword",
      "keywords": ["客户", "数据"]
    },
    {
      "type": "regex",
      "pattern": "\\d{11}"
    }
  ]
}
```

**支持的操作符**：
- `AND`：所有规则都匹配
- `OR`：任一规则匹配
- `NOT`：规则不匹配
- `XOR`：恰好一个规则匹配

### 规则优先级

当多个规则匹配时，按以下优先级处理：

1. **严重级别**：Critical > High > Medium > Low
2. **动作类型**：Block > Redact > Warn
3. **规则顺序**：先定义的规则优先

## 数据结构设计

### Rust 数据结构

```rust
/// DLP 规则类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DlpRuleType {
    /// 正则表达式
    Regex {
        pattern: String,
        flags: Option<String>,
    },
    
    /// 关键字匹配
    Keyword {
        keywords: Vec<String>,
        match_mode: KeywordMatchMode,
        case_sensitive: bool,
    },
    
    /// 字典匹配
    Dictionary {
        dictionary_id: String,
        dictionary_source: String,
        match_mode: KeywordMatchMode,
    },
    
    /// 数据库指纹
    DatabaseFingerprint {
        detect_sql_statements: bool,
        detect_table_structures: bool,
        sensitive_column_names: Vec<String>,
        min_rows: usize,
    },
    
    /// 文档指纹
    DocumentFingerprint {
        fingerprint_method: FingerprintMethod,
        document_hashes: Vec<String>,
        similarity_threshold: f32,
    },
    
    /// 图像识别
    ImageRecognition {
        detection_methods: Vec<ImageDetectionMethod>,
        ocr_languages: Vec<String>,
        min_confidence: f32,
    },
    
    /// 语义模型
    SemanticModel {
        model_type: SemanticModelType,
        model_path: String,
        categories: Vec<String>,
        min_confidence: f32,
    },
    
    /// 文件类型
    FileType {
        allowed_types: Option<Vec<String>>,
        blocked_types: Option<Vec<String>>,
        check_magic_number: bool,
    },
    
    /// 文件标签
    FileLabel {
        sensitive_labels: Vec<String>,
        label_sources: Vec<LabelSource>,
    },
    
    /// 组合规则
    Composite {
        operator: CompositeOperator,
        rules: Vec<Box<DlpRuleType>>,
    },
}

/// 关键字匹配模式
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeywordMatchMode {
    Exact,
    Prefix,
    Suffix,
    Contains,
    WholeWord,
}

/// 指纹方法
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FingerprintMethod {
    Md5,
    Sha256,
    FuzzyHash,
}

/// 图像检测方法
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageDetectionMethod {
    Ocr,
    FaceDetection,
    ObjectDetection,
    SceneRecognition,
}

/// 语义模型类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticModelType {
    TextClassification,
    SentimentAnalysis,
    EntityRecognition,
    IntentDetection,
}

/// 标签来源
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelSource {
    Filesystem,
    Metadata,
    Custom,
}

/// 组合操作符
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompositeOperator {
    And,
    Or,
    Not,
    Xor,
}

/// 完整的 DLP 规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlpRule {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub rule_type: DlpRuleType,
    pub severity: Severity,
    pub action: PolicyAction,
    pub category: String,
    pub enabled: bool,
    pub created_by: Uuid,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

## 实施计划

### 阶段 1：基础扩展（1-2 周）

- [ ] 实现关键字匹配
- [ ] 实现字典匹配（Aho-Corasick）
- [ ] 更新数据库 schema
- [ ] 更新前端 UI

### 阶段 2：指纹检测（2-3 周）

- [ ] 实现数据库指纹检测
- [ ] 实现文档指纹检测
- [ ] 实现文件类型检测
- [ ] 实现文件标签检测

### 阶段 3：高级检测（3-4 周）

- [ ] 集成 OCR 引擎（Tesseract）
- [ ] 实现图像识别
- [ ] 集成语义模型（ONNX Runtime）
- [ ] 实现组合规则

### 阶段 4：优化和测试（1-2 周）

- [ ] 性能优化
- [ ] 完整的测试覆盖
- [ ] 文档和示例
- [ ] 用户培训

## 性能考虑

### 检测性能

| 规则类型 | 性能 | 适用场景 |
|---------|------|---------|
| 正则表达式 | 中等 | 结构化数据 |
| 关键字 | 快 | 简单词汇 |
| 字典 | 快 | 大量词汇 |
| 数据库指纹 | 快 | 结构化数据 |
| 文档指纹 | 中等 | 文档检测 |
| 图像识别 | 慢 | 图像内容 |
| 语义模型 | 慢 | 复杂语义 |
| 文件类型 | 快 | 文件过滤 |
| 文件标签 | 快 | 标签检测 |

### 优化策略

1. **分层检测**：先快速检测，再精确检测
2. **缓存结果**：缓存常见内容的检测结果
3. **并行处理**：多个规则并行检测
4. **采样检测**：大文件采样检测
5. **异步检测**：非阻塞检测

## 参考资料

- [Aho-Corasick 算法](https://en.wikipedia.org/wiki/Aho%E2%80%93Corasick_algorithm)
- [Fuzzy Hashing](https://ssdeep-project.github.io/ssdeep/index.html)
- [Tesseract OCR](https://github.com/tesseract-ocr/tesseract)
- [ONNX Runtime](https://onnxruntime.ai/)
