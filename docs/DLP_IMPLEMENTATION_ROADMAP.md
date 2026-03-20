# DLP 多维度规则系统实施路线图

## 当前状态

### 已实现功能 ✅

- 正则表达式匹配
- 基本的规则管理（CRUD）
- 规则启用/禁用
- 严重级别和分类
- 前端管理界面

### 缺失功能 ❌

- 关键字匹配
- 字典匹配
- 数据库指纹
- 文档指纹
- 图像识别
- 语义模型
- 文件类型检测
- 文件标签检测
- 组合规则

## 实施路线图

### 阶段 1：基础扩展（优先级：高）

**目标**：扩展基础的文本匹配能力

**时间**：1-2 周

**任务**：

#### 1.1 关键字匹配

**后端实现**：

```rust
// crates/ironclaw_safety/src/keyword_matcher.rs

use aho_corasick::AhoCorasick;

#[derive(Debug, Clone)]
pub enum KeywordMatchMode {
    Exact,
    Prefix,
    Suffix,
    Contains,
    WholeWord,
}

pub struct KeywordMatcher {
    keywords: Vec<String>,
    mode: KeywordMatchMode,
    case_sensitive: bool,
    ac: Option<AhoCorasick>,
}

impl KeywordMatcher {
    pub fn new(keywords: Vec<String>, mode: KeywordMatchMode, case_sensitive: bool) -> Self {
        let ac = if mode == KeywordMatchMode::Exact || mode == KeywordMatchMode::Contains {
            Some(AhoCorasick::new(&keywords).unwrap())
        } else {
            None
        };
        
        Self {
            keywords,
            mode,
            case_sensitive,
            ac,
        }
    }
    
    pub fn find_matches(&self, content: &str) -> Vec<KeywordMatch> {
        match self.mode {
            KeywordMatchMode::Exact | KeywordMatchMode::Contains => {
                self.find_with_aho_corasick(content)
            }
            KeywordMatchMode::Prefix => self.find_with_prefix(content),
            KeywordMatchMode::Suffix => self.find_with_suffix(content),
            KeywordMatchMode::WholeWord => self.find_whole_words(content),
        }
    }
}
```

**前端实现**：

```typescript
// admin-backend/frontend/src/types/dlp.ts

export type DlpRuleType = 
  | { type: 'regex'; pattern: string; flags?: string }
  | { type: 'keyword'; keywords: string[]; matchMode: KeywordMatchMode; caseSensitive: boolean }
  | { type: 'dictionary'; dictionaryId: string; matchMode: KeywordMatchMode };

export type KeywordMatchMode = 'exact' | 'prefix' | 'suffix' | 'contains' | 'whole_word';
```

**UI 组件**：

```tsx
// admin-backend/frontend/src/components/Security/KeywordRuleForm.tsx

export function KeywordRuleForm() {
  const [keywords, setKeywords] = useState<string[]>([]);
  const [matchMode, setMatchMode] = useState<KeywordMatchMode>('exact');
  const [caseSensitive, setCaseSensitive] = useState(false);
  
  return (
    <div>
      <TagInput
        label="关键字列表"
        value={keywords}
        onChange={setKeywords}
        placeholder="输入关键字后按回车"
      />
      
      <Select
        label="匹配模式"
        value={matchMode}
        onChange={setMatchMode}
        options={[
          { value: 'exact', label: '精确匹配' },
          { value: 'prefix', label: '前缀匹配' },
          { value: 'suffix', label: '后缀匹配' },
          { value: 'contains', label: '包含匹配' },
          { value: 'whole_word', label: '全词匹配' },
        ]}
      />
      
      <Checkbox
        label="区分大小写"
        checked={caseSensitive}
        onChange={setCaseSensitive}
      />
    </div>
  );
}
```

#### 1.2 字典匹配

**后端实现**：

```rust
// crates/ironclaw_safety/src/dictionary_matcher.rs

use aho_corasick::AhoCorasick;
use std::path::Path;

pub struct DictionaryMatcher {
    dictionary_id: String,
    ac: AhoCorasick,
    keywords: Vec<String>,
}

impl DictionaryMatcher {
    pub fn from_file(dictionary_id: String, path: &Path) -> Result<Self, Error> {
        let content = std::fs::read_to_string(path)?;
        let keywords: Vec<String> = content
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| line.trim().to_string())
            .collect();
        
        let ac = AhoCorasick::new(&keywords)?;
        
        Ok(Self {
            dictionary_id,
            ac,
            keywords,
        })
    }
    
    pub fn find_matches(&self, content: &str) -> Vec<DictionaryMatch> {
        self.ac
            .find_iter(content)
            .map(|mat| DictionaryMatch {
                keyword: self.keywords[mat.pattern()].clone(),
                start: mat.start(),
                end: mat.end(),
            })
            .collect()
    }
}
```

**字典管理 API**：

```rust
// admin-backend/src/dictionary_management.rs

pub async fn create_dictionary(
    db: &Database,
    name: String,
    description: Option<String>,
    keywords: Vec<String>,
) -> Result<Dictionary, Error> {
    // 保存字典到数据库
    // 生成字典文件
}

pub async fn upload_dictionary_file(
    db: &Database,
    name: String,
    file_path: &Path,
) -> Result<Dictionary, Error> {
    // 上传字典文件
    // 解析并验证
}
```

#### 1.3 数据库 Schema 更新

```sql
-- migrations/add_dlp_rule_types.sql

-- 添加规则类型字段
ALTER TABLE dlp_rules ADD COLUMN rule_type TEXT NOT NULL DEFAULT 'regex';
ALTER TABLE dlp_rules ADD COLUMN rule_config JSONB;

-- 创建字典表
CREATE TABLE dlp_dictionaries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    file_path TEXT NOT NULL,
    keyword_count INTEGER NOT NULL,
    created_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 创建字典索引
CREATE INDEX idx_dlp_dictionaries_name ON dlp_dictionaries(name);
```

**检查清单**：

- [ ] 实现 KeywordMatcher
- [ ] 实现 DictionaryMatcher
- [ ] 更新数据库 schema
- [ ] 实现字典管理 API
- [ ] 更新前端类型定义
- [ ] 实现前端 UI 组件
- [ ] 编写单元测试
- [ ] 编写集成测试
- [ ] 更新文档

### 阶段 2：指纹检测（优先级：中）

**目标**：实现基于指纹的检测能力

**时间**：2-3 周

**任务**：

#### 2.1 数据库指纹检测

**检测 SQL 语句**：

```rust
// crates/ironclaw_safety/src/database_fingerprint.rs

pub struct DatabaseFingerprintDetector {
    sql_patterns: Vec<Regex>,
    sensitive_column_names: Vec<String>,
    min_rows: usize,
}

impl DatabaseFingerprintDetector {
    pub fn detect(&self, content: &str) -> Vec<DatabaseMatch> {
        let mut matches = Vec::new();
        
        // 检测 SQL 语句
        for pattern in &self.sql_patterns {
            for mat in pattern.find_iter(content) {
                matches.push(DatabaseMatch {
                    match_type: DatabaseMatchType::SqlStatement,
                    content: mat.as_str().to_string(),
                    location: mat.range(),
                });
            }
        }
        
        // 检测表结构
        if self.detect_table_structure(content) {
            matches.push(DatabaseMatch {
                match_type: DatabaseMatchType::TableStructure,
                content: content.to_string(),
                location: 0..content.len(),
            });
        }
        
        matches
    }
    
    fn detect_table_structure(&self, content: &str) -> bool {
        // 检测是否包含敏感列名
        let sensitive_columns_found = self.sensitive_column_names
            .iter()
            .filter(|col| content.contains(*col))
            .count();
        
        // 检测是否有足够的行数
        let row_count = content.lines().count();
        
        sensitive_columns_found >= 2 && row_count >= self.min_rows
    }
}
```

#### 2.2 文档指纹检测

**Fuzzy Hash 实现**：

```rust
// crates/ironclaw_safety/src/document_fingerprint.rs

use ssdeep::hash;

pub struct DocumentFingerprintDetector {
    document_hashes: Vec<String>,
    similarity_threshold: f32,
}

impl DocumentFingerprintDetector {
    pub fn detect(&self, content: &[u8]) -> Option<DocumentMatch> {
        let content_hash = hash(content).ok()?;
        
        for (idx, doc_hash) in self.document_hashes.iter().enumerate() {
            let similarity = ssdeep::compare(&content_hash, doc_hash).ok()?;
            
            if similarity >= (self.similarity_threshold * 100.0) as u32 {
                return Some(DocumentMatch {
                    document_index: idx,
                    similarity: similarity as f32 / 100.0,
                    hash: content_hash,
                });
            }
        }
        
        None
    }
}
```

#### 2.3 文件类型检测

```rust
// crates/ironclaw_safety/src/file_type_detector.rs

use infer::Infer;

pub struct FileTypeDetector {
    allowed_types: Option<Vec<String>>,
    blocked_types: Option<Vec<String>>,
    check_magic_number: bool,
}

impl FileTypeDetector {
    pub fn detect(&self, content: &[u8], filename: &str) -> FileTypeResult {
        let detected_type = if self.check_magic_number {
            let infer = Infer::new();
            infer.get(content).map(|t| t.mime_type().to_string())
        } else {
            None
        };
        
        let extension = Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());
        
        // 检查是否允许
        if let Some(allowed) = &self.allowed_types {
            if let Some(ref mime) = detected_type {
                if !allowed.contains(mime) {
                    return FileTypeResult::Blocked(format!("文件类型 {} 不允许", mime));
                }
            }
        }
        
        // 检查是否禁止
        if let Some(blocked) = &self.blocked_types {
            if let Some(ref mime) = detected_type {
                if blocked.contains(mime) {
                    return FileTypeResult::Blocked(format!("文件类型 {} 被禁止", mime));
                }
            }
        }
        
        FileTypeResult::Allowed
    }
}
```

**检查清单**：

- [ ] 实现 DatabaseFingerprintDetector
- [ ] 实现 DocumentFingerprintDetector
- [ ] 实现 FileTypeDetector
- [ ] 实现 FileLabelDetector
- [ ] 更新前端 UI
- [ ] 编写测试
- [ ] 更新文档

### 阶段 3：高级检测（优先级：低）

**目标**：实现基于 AI 的高级检测

**时间**：3-4 周

**任务**：

#### 3.1 图像识别（OCR）

**依赖**：
- Tesseract OCR
- leptonica

**实现**：

```rust
// crates/ironclaw_safety/src/image_recognition.rs

use tesseract::Tesseract;

pub struct ImageRecognitionDetector {
    ocr_languages: Vec<String>,
    min_confidence: f32,
}

impl ImageRecognitionDetector {
    pub fn detect(&self, image_data: &[u8]) -> Result<Vec<OcrMatch>, Error> {
        let mut tesseract = Tesseract::new(None, Some(&self.ocr_languages.join("+")))?;
        tesseract.set_image_from_mem(image_data)?;
        
        let text = tesseract.get_text()?;
        let confidence = tesseract.mean_text_conf();
        
        if confidence >= (self.min_confidence * 100.0) as i32 {
            Ok(vec![OcrMatch {
                text,
                confidence: confidence as f32 / 100.0,
            }])
        } else {
            Ok(vec![])
        }
    }
}
```

#### 3.2 语义模型

**依赖**：
- ONNX Runtime
- Tokenizers

**实现**：

```rust
// crates/ironclaw_safety/src/semantic_model.rs

use ort::{Environment, Session, Value};

pub struct SemanticModelDetector {
    session: Session,
    categories: Vec<String>,
    min_confidence: f32,
}

impl SemanticModelDetector {
    pub fn detect(&self, text: &str) -> Result<Vec<SemanticMatch>, Error> {
        // Tokenize
        let input_ids = self.tokenize(text)?;
        
        // Run inference
        let outputs = self.session.run(vec![Value::from_array(
            self.session.allocator(),
            &input_ids,
        )?])?;
        
        // Parse results
        let logits = outputs[0].try_extract::<f32>()?;
        let probabilities = softmax(&logits);
        
        let mut matches = Vec::new();
        for (idx, &prob) in probabilities.iter().enumerate() {
            if prob >= self.min_confidence {
                matches.push(SemanticMatch {
                    category: self.categories[idx].clone(),
                    confidence: prob,
                });
            }
        }
        
        Ok(matches)
    }
}
```

**检查清单**：

- [ ] 集成 Tesseract OCR
- [ ] 实现 ImageRecognitionDetector
- [ ] 集成 ONNX Runtime
- [ ] 实现 SemanticModelDetector
- [ ] 训练或获取预训练模型
- [ ] 更新前端 UI
- [ ] 编写测试
- [ ] 性能优化
- [ ] 更新文档

### 阶段 4：组合规则和优化（优先级：中）

**目标**：支持复杂的规则组合和性能优化

**时间**：1-2 周

**任务**：

#### 4.1 组合规则

```rust
// crates/ironclaw_safety/src/composite_rule.rs

pub enum CompositeOperator {
    And,
    Or,
    Not,
    Xor,
}

pub struct CompositeRule {
    operator: CompositeOperator,
    rules: Vec<Box<dyn DlpDetector>>,
}

impl CompositeRule {
    pub fn detect(&self, content: &str) -> Vec<Match> {
        match self.operator {
            CompositeOperator::And => {
                // 所有规则都匹配
                let mut all_matches = Vec::new();
                for rule in &self.rules {
                    let matches = rule.detect(content);
                    if matches.is_empty() {
                        return vec![];
                    }
                    all_matches.extend(matches);
                }
                all_matches
            }
            CompositeOperator::Or => {
                // 任一规则匹配
                let mut all_matches = Vec::new();
                for rule in &self.rules {
                    all_matches.extend(rule.detect(content));
                }
                all_matches
            }
            // ... 其他操作符
        }
    }
}
```

#### 4.2 性能优化

**分层检测**：

```rust
pub struct LayeredDetector {
    fast_detectors: Vec<Box<dyn DlpDetector>>,
    slow_detectors: Vec<Box<dyn DlpDetector>>,
}

impl LayeredDetector {
    pub fn detect(&self, content: &str) -> Vec<Match> {
        // 先运行快速检测器
        let mut matches = Vec::new();
        for detector in &self.fast_detectors {
            matches.extend(detector.detect(content));
        }
        
        // 如果快速检测器有匹配，再运行慢速检测器
        if !matches.is_empty() {
            for detector in &self.slow_detectors {
                matches.extend(detector.detect(content));
            }
        }
        
        matches
    }
}
```

**并行检测**：

```rust
use rayon::prelude::*;

pub fn detect_parallel(
    detectors: &[Box<dyn DlpDetector + Sync>],
    content: &str,
) -> Vec<Match> {
    detectors
        .par_iter()
        .flat_map(|detector| detector.detect(content))
        .collect()
}
```

**检查清单**：

- [ ] 实现 CompositeRule
- [ ] 实现分层检测
- [ ] 实现并行检测
- [ ] 实现结果缓存
- [ ] 性能基准测试
- [ ] 优化热点代码
- [ ] 更新文档

## 测试策略

### 单元测试

每个检测器都需要完整的单元测试：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_keyword_matcher_exact() {
        let matcher = KeywordMatcher::new(
            vec!["机密".to_string(), "绝密".to_string()],
            KeywordMatchMode::Exact,
            false,
        );
        
        let matches = matcher.find_matches("这是机密文件");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].keyword, "机密");
    }
    
    #[test]
    fn test_keyword_matcher_whole_word() {
        let matcher = KeywordMatcher::new(
            vec!["password".to_string()],
            KeywordMatchMode::WholeWord,
            false,
        );
        
        let matches = matcher.find_matches("my password is secret");
        assert_eq!(matches.len(), 1);
        
        let matches = matcher.find_matches("my passwords are secret");
        assert_eq!(matches.len(), 0); // 不匹配复数形式
    }
}
```

### 集成测试

测试完整的检测流程：

```rust
#[tokio::test]
async fn test_dlp_detection_pipeline() {
    let detector = DlpDetector::new()
        .with_regex_rules(vec![...])
        .with_keyword_rules(vec![...])
        .with_dictionary_rules(vec![...]);
    
    let content = "我的身份证号是 330326199408015618，密码是 password123";
    let result = detector.scan(content).await.unwrap();
    
    assert!(result.had_sensitive_data);
    assert_eq!(result.matches.len(), 2); // 身份证 + 密码
}
```

### 性能测试

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_keyword_matcher(c: &mut Criterion) {
    let matcher = KeywordMatcher::new(
        (0..1000).map(|i| format!("keyword{}", i)).collect(),
        KeywordMatchMode::Exact,
        false,
    );
    
    let content = "这是一个包含 keyword500 的测试文本";
    
    c.bench_function("keyword_matcher_1000_keywords", |b| {
        b.iter(|| matcher.find_matches(black_box(content)))
    });
}

criterion_group!(benches, benchmark_keyword_matcher);
criterion_main!(benches);
```

## 文档更新

需要更新的文档：

- [ ] `DLP_RULE_MANAGEMENT_COMPLETE.md` - 添加新规则类型
- [ ] `API.md` - 更新 API 文档
- [ ] `USER_GUIDE.md` - 用户使用指南
- [ ] `DEVELOPER_GUIDE.md` - 开发者指南
- [ ] `PERFORMANCE.md` - 性能优化指南

## 总结

这个路线图提供了一个完整的 DLP 多维度规则系统的实施计划。建议按照优先级逐步实施：

1. **阶段 1（高优先级）**：关键字和字典匹配 - 快速提升检测能力
2. **阶段 2（中优先级）**：指纹检测 - 增强结构化数据检测
3. **阶段 4（中优先级）**：组合规则和优化 - 提升灵活性和性能
4. **阶段 3（低优先级）**：AI 检测 - 高级功能，需要更多资源

每个阶段都应该包含完整的测试和文档，确保质量和可维护性。
