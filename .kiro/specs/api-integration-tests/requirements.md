# 需求文档：Desktop Client API 集成测试

## 介绍

本文档定义了 desktop-client 的服务端接口集成测试需求。当前 desktop-client 通过 `api_client.rs` 调用后端 Web Gateway API，但基础流程存在参数名称错误、URL 错误等问题，导致接口无法正常工作。本功能旨在添加完整的集成测试，确保所有 API 端点的正确性、参数格式、错误处理和基础流程的正常运行。

## 术语表

- **Desktop_Client**: 基于 Tauri 的桌面应用程序，通过 HTTP 调用后端服务
- **Web_Gateway**: 后端 Web 服务，提供 RESTful API 端点（位于 `src/channels/web/`）
- **API_Client**: desktop-client 中的 HTTP 客户端模块（`desktop-client/src/api_client.rs`）
- **Integration_Test**: 集成测试，验证 API_Client 与 Web_Gateway 之间的端到端通信
- **Test_Server**: 用于集成测试的模拟或真实 Web_Gateway 实例
- **Round_Trip**: 往返测试，验证数据序列化、传输、反序列化的完整流程

## 需求

### 需求 1: 聊天接口集成测试

**用户故事**: 作为开发者，我希望验证所有聊天相关的 API 接口都能正常工作，以便用户可以创建对话、发送消息和管理聊天历史。

#### 验收标准

1. WHEN 调用 `get_threads` 接口时，THE API_Client SHALL 正确解析 ThreadListResponse 并返回对话列表
2. WHEN 调用 `create_thread` 接口时，THE API_Client SHALL 成功创建新对话并返回 ThreadInfo
3. WHEN 调用 `send_message` 接口时，THE API_Client SHALL 正确发送消息内容和 thread_id 参数
4. WHEN 调用 `get_messages` 接口时，THE API_Client SHALL 正确解析历史消息并转换为 Message 列表
5. WHEN 调用 `search_messages` 接口时，THE API_Client SHALL 正确编码查询参数并返回搜索结果
6. WHEN 调用 `edit_message` 接口时，THE API_Client SHALL 正确更新消息内容
7. WHEN 调用 `delete_message` 接口时，THE API_Client SHALL 成功删除指定消息
8. WHEN 调用 `export_thread` 接口时，THE API_Client SHALL 返回指定格式的导出数据
9. WHEN 调用 `upload_file` 接口时，THE API_Client SHALL 成功上传文件并返回文件 ID
10. FOR ALL 聊天接口，解析响应然后序列化再解析 SHALL 产生等效的数据结构（round-trip 属性）

### 需求 2: 记忆接口集成测试

**用户故事**: 作为开发者，我希望验证记忆管理相关的 API 接口都能正常工作，以便用户可以浏览、读取、写入和搜索工作区文件。

#### 验收标准

1. WHEN 调用 `get_memory_tree` 接口时，THE API_Client SHALL 正确解析树形结构并返回 MemoryTreeResponse
2. WHEN 调用 `read_memory` 接口时，THE API_Client SHALL 返回指定 memory_id 的内容
3. WHEN 调用 `write_memory` 接口时，THE API_Client SHALL 成功写入内容并返回更新后的 MemoryContent
4. WHEN 调用 `search_memory` 接口时，THE API_Client SHALL 正确编码查询参数并返回匹配的记忆列表
5. FOR ALL 记忆接口，解析响应然后序列化再解析 SHALL 产生等效的数据结构（round-trip 属性）

### 需求 3: 任务接口集成测试

**用户故事**: 作为开发者，我希望验证任务管理相关的 API 接口都能正常工作，以便用户可以查看、取消和重启任务。

#### 验收标准

1. WHEN 调用 `get_jobs` 接口时，THE API_Client SHALL 返回任务列表
2. WHEN 调用 `get_job_detail` 接口时，THE API_Client SHALL 返回指定任务的详细信息
3. WHEN 调用 `cancel_job` 接口时，THE API_Client SHALL 成功取消指定任务
4. WHEN 调用 `restart_job` 接口时，THE API_Client SHALL 成功重启指定任务
5. FOR ALL 任务接口，解析响应然后序列化再解析 SHALL 产生等效的数据结构（round-trip 属性）

### 需求 4: 日志接口集成测试

**用户故事**: 作为开发者，我希望验证日志查询相关的 API 接口都能正常工作，以便用户可以查看、搜索、过滤和导出日志。

#### 验收标准

1. WHEN 调用 `get_logs` 接口时，THE API_Client SHALL 返回指定数量的日志条目
2. WHEN 调用 `search_logs` 接口时，THE API_Client SHALL 正确编码查询参数并返回匹配的日志
3. WHEN 调用 `filter_logs` 接口时，THE API_Client SHALL 正确传递 level 和 module 参数
4. WHEN 调用 `export_logs` 接口时，THE API_Client SHALL 返回指定格式的导出数据
5. WHEN 调用 `clear_logs` 接口时，THE API_Client SHALL 成功清空日志
6. FOR ALL 日志接口，解析响应然后序列化再解析 SHALL 产生等效的数据结构（round-trip 属性）

### 需求 5: 批准接口集成测试

**用户故事**: 作为开发者，我希望验证操作批准相关的 API 接口都能正常工作，以便用户可以批准或拒绝敏感操作。

#### 验收标准

1. WHEN 调用 `approve_operation` 接口时，THE API_Client SHALL 正确传递 request_id、action 和 thread_id 参数
2. WHEN 调用 `deny_operation` 接口时，THE API_Client SHALL 正确传递 request_id 和 thread_id 参数
3. FOR ALL 批准接口，解析响应然后序列化再解析 SHALL 产生等效的数据结构（round-trip 属性）

### 需求 6: URL 和参数正确性验证

**用户故事**: 作为开发者，我希望验证所有 API 调用使用正确的 URL 路径和参数名称，以便避免 404 错误和参数不匹配问题。

#### 验收标准

1. FOR ALL API 端点，THE API_Client SHALL 使用与 Web_Gateway 路由定义一致的 URL 路径
2. FOR ALL API 请求，THE API_Client SHALL 使用与 Web_Gateway 处理器期望一致的参数名称
3. WHEN API 端点需要路径参数时，THE API_Client SHALL 正确构造 URL（如 `/api/jobs/{id}`）
4. WHEN API 端点需要查询参数时，THE API_Client SHALL 正确编码查询字符串
5. WHEN API 端点需要请求体时，THE API_Client SHALL 使用正确的 JSON 字段名称

### 需求 7: 错误处理测试

**用户故事**: 作为开发者，我希望验证 API 客户端能够正确处理各种错误情况，以便向用户提供清晰的错误信息。

#### 验收标准

1. WHEN Web_Gateway 返回 4xx 错误时，THE API_Client SHALL 返回包含错误信息的 Error
2. WHEN Web_Gateway 返回 5xx 错误时，THE API_Client SHALL 返回包含错误信息的 Error
3. WHEN 网络连接失败时，THE API_Client SHALL 返回网络错误
4. WHEN 响应 JSON 解析失败时，THE API_Client SHALL 返回序列化错误
5. WHEN API 调用超时时，THE API_Client SHALL 返回超时错误

### 需求 8: 认证和授权测试

**用户故事**: 作为开发者，我希望验证 API 客户端正确处理认证令牌，以便确保安全的 API 访问。

#### 验收标准

1. FOR ALL 受保护的 API 端点，THE API_Client SHALL 在请求头中包含 Authorization Bearer token
2. WHEN 认证令牌无效时，THE API_Client SHALL 返回认证错误
3. WHEN 认证令牌缺失时，THE API_Client SHALL 返回认证错误

### 需求 9: 数据类型和序列化测试

**用户故事**: 作为开发者，我希望验证所有数据结构能够正确序列化和反序列化，以便确保数据完整性。

#### 验收标准

1. FOR ALL 请求数据结构，序列化为 JSON 然后反序列化 SHALL 产生等效的结构（round-trip 属性）
2. FOR ALL 响应数据结构，反序列化 JSON 然后序列化 SHALL 产生等效的 JSON（round-trip 属性）
3. WHEN 响应包含可选字段时，THE API_Client SHALL 正确处理 null 值
4. WHEN 响应包含嵌套结构时，THE API_Client SHALL 正确解析所有层级
5. WHEN 响应包含数组时，THE API_Client SHALL 正确解析所有元素

### 需求 10: 基础流程端到端测试

**用户故事**: 作为开发者，我希望验证常见的用户操作流程能够端到端正常工作，以便确保基本功能可用。

#### 验收标准

1. THE Integration_Test SHALL 验证"创建对话 → 发送消息 → 获取历史"流程
2. THE Integration_Test SHALL 验证"获取记忆树 → 读取文件 → 写入文件"流程
3. THE Integration_Test SHALL 验证"获取任务列表 → 获取任务详情"流程
4. THE Integration_Test SHALL 验证"获取日志 → 搜索日志 → 过滤日志"流程
5. THE Integration_Test SHALL 验证"发送消息 → 批准操作"流程

### 需求 11: 测试基础设施

**用户故事**: 作为开发者，我希望有可靠的测试基础设施，以便能够自动化运行集成测试。

#### 验收标准

1. THE Test_Infrastructure SHALL 提供启动和停止 Test_Server 的辅助函数
2. THE Test_Infrastructure SHALL 提供创建测试数据的辅助函数
3. THE Test_Infrastructure SHALL 提供清理测试数据的辅助函数
4. THE Test_Infrastructure SHALL 支持并行运行多个测试用例
5. THE Test_Infrastructure SHALL 在测试失败时提供详细的错误信息

### 需求 12: 性能和可靠性测试

**用户故事**: 作为开发者，我希望验证 API 客户端在各种条件下都能可靠工作，以便确保生产环境的稳定性。

#### 验收标准

1. WHEN 发送大量并发请求时，THE API_Client SHALL 正确处理所有响应
2. WHEN 发送大尺寸请求体时（如长消息），THE API_Client SHALL 成功传输数据
3. WHEN 接收大尺寸响应时（如长日志列表），THE API_Client SHALL 成功解析数据
4. WHEN 网络延迟较高时，THE API_Client SHALL 在合理时间内完成请求或超时
5. WHEN 连续发送多个请求时，THE API_Client SHALL 保持连接复用以提高性能

## 特殊需求指导

### Parser 和 Serializer 需求

本功能涉及大量的 JSON 序列化和反序列化操作，因此必须包含完整的 round-trip 测试：

1. **请求序列化测试**：
   - 所有请求结构（SendMessageRequest、ApprovalRequest 等）必须能够序列化为 JSON
   - 序列化后的 JSON 必须包含所有必需字段
   - 反序列化序列化后的 JSON 必须产生等效的结构

2. **响应反序列化测试**：
   - 所有响应结构（ThreadListResponse、Message、JobInfo 等）必须能够从 JSON 反序列化
   - 反序列化后序列化再反序列化必须产生等效的结构
   - 必须正确处理可选字段（Option<T>）

3. **Round-trip 属性测试**：
   ```rust
   // 对于每个数据结构 T
   fn round_trip_property<T: Serialize + DeserializeOwned + PartialEq>(value: T) {
       let json = serde_json::to_string(&value).unwrap();
       let parsed: T = serde_json::from_str(&json).unwrap();
       assert_eq!(value, parsed);
   }
   ```

### 错误条件测试

必须测试所有可能的错误情况：

1. **网络错误**：连接失败、超时、连接中断
2. **HTTP 错误**：400、401、403、404、500 等状态码
3. **数据错误**：JSON 解析失败、字段缺失、类型不匹配
4. **业务错误**：资源不存在、权限不足、操作冲突

### 测试覆盖率要求

1. 所有 API_Client 中的公共方法必须有对应的集成测试
2. 所有错误路径必须有对应的测试用例
3. 所有数据结构必须有 round-trip 测试
4. 所有端到端流程必须有集成测试

## 实现注意事项

1. **测试隔离**：每个测试用例应该独立运行，不依赖其他测试的状态
2. **测试数据**：使用确定性的测试数据，避免随机性导致的测试不稳定
3. **清理机制**：测试结束后应该清理所有创建的资源
4. **并行执行**：测试应该支持并行执行以提高效率
5. **错误信息**：测试失败时应该提供清晰的错误信息，包括期望值和实际值

## 成功标准

1. 所有 API 端点都有对应的集成测试
2. 所有测试用例都能通过
3. 测试覆盖率达到 90% 以上
4. 基础流程能够端到端正常运行
5. 错误处理能够正确工作
6. 文档完整，包括测试用例说明和运行指南
