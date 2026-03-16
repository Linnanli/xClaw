# 实现计划：Desktop Client API 集成测试

## 概述

本实现计划将为 desktop-client 的 API 客户端建立完整的集成测试框架。测试将验证 `api_client.rs` 与 Web Gateway 之间的端到端通信，确保所有 API 端点的 URL、参数名称、数据格式和错误处理都正确无误。

实现采用 TDD 方法，结合单元测试和属性测试（property-based testing），使用真实的 Web Gateway 实例和内存数据库进行测试隔离。

## 任务

- [x] 1. 建立测试基础设施
  - [x] 1.1 实现 TestServer 组件
    - 创建 `desktop-client/tests/support/test_server.rs`
    - 实现 TestServer 结构体，支持启动/停止 Web Gateway
    - 实现动态端口分配（bind to 0）
    - 实现 RAII 模式的自动清理（Drop trait）
    - 提供 `create_client()` 方法返回配置好的 ApiClient
    - _需求: 11.1, 11.4_

  - [x]* 1.2 为 TestServer 编写单元测试
    - 测试服务器启动和停止
    - 测试动态端口分配
    - 测试自动清理机制
    - _需求: 11.1, 11.5_

  - [x] 1.3 实现 TestFixture 组件
    - 创建 `desktop-client/tests/support/test_fixture.rs`
    - 实现 TestFixture 结构体，提供测试数据创建和清理
    - 实现 `create_thread()` 辅助方法
    - 实现 `send_message()` 辅助方法
    - 实现 RAII 模式的资源清理
    - _需求: 11.2, 11.3_

  - [x]* 1.4 为 TestFixture 编写单元测试
    - 测试测试数据创建
    - 测试资源清理机制
    - _需求: 11.2, 11.3, 11.5_

  - [x] 1.5 实现 Proptest 数据生成器
    - 创建 `desktop-client/tests/support/generators.rs`
    - 实现 `arb_message_content()` 生成器
    - 实现 `arb_thread_id()` 生成器
    - 实现 `arb_send_message_request()` 生成器
    - 实现其他请求/响应类型的生成器
    - _需求: 1.10, 2.5, 3.5, 4.6, 5.3_

  - [x] 1.6 创建测试支持模块入口
    - 创建 `desktop-client/tests/support/mod.rs`
    - 重导出 TestServer、TestFixture 和生成器
    - 定义 TestConfig 和 TestError 类型
    - _需求: 11.1_

- [x] 2. 实现聊天接口集成测试
  - [x] 2.1 实现聊天接口单元测试
    - 创建 `desktop-client/tests/api_integration_tests.rs`
    - 测试 `get_threads()` 接口
    - 测试 `create_thread()` 接口
    - 测试 `send_message()` 接口
    - 测试 `get_messages()` 接口
    - 测试 `search_messages()` 接口（含特殊字符编码）
    - 测试 `edit_message()` 接口
    - 测试 `delete_message()` 接口
    - 测试 `export_thread()` 接口
    - 测试 `upload_file()` 接口
    - _需求: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 1.8, 1.9, 6.1, 6.2, 6.3, 6.4, 6.5_

  - [x]* 2.2 编写聊天接口属性测试
    - 创建 `desktop-client/tests/api_property_tests.rs`
    - **属性 1: 数据序列化 Round-trip**
    - **验证需求: 1.10**
    - 使用 proptest 生成随机 SendMessageRequest
    - 验证序列化后反序列化产生等效结构
    - _需求: 1.10, 9.1, 9.2_

  - [x]* 2.3 编写聊天接口成功返回属性测试
    - **属性 2: API 调用成功返回有效数据**
    - **验证需求: 1.1, 1.2, 1.4**
    - 使用 proptest 生成随机有效请求
    - 验证所有调用成功并返回预期类型
    - _需求: 1.1, 1.2, 1.4_

  - [x]* 2.4 编写聊天接口幂等性属性测试
    - **属性 3: 写入操作的幂等性验证**
    - **验证需求: 1.2, 1.3, 1.6, 1.7**
    - 验证创建、编辑、删除操作后查询反映变更
    - _需求: 1.2, 1.3, 1.6, 1.7_

  - [x]* 2.5 编写查询参数编码属性测试
    - **属性 4: 查询参数正确编码**
    - **验证需求: 1.5**
    - 使用包含特殊字符的查询参数
    - 验证 URL 编码正确且服务器能解析
    - _需求: 1.5_

- [x] 3. 检查点 - 验证聊天接口测试
  - 运行 `cargo test --test api_integration_tests`
  - 运行 `cargo test --test api_property_tests`
  - 确保所有测试通过，询问用户是否有问题

- [ ] 4. 实现记忆接口集成测试
  - [ ] 4.1 实现记忆接口单元测试
    - 在 `api_integration_tests.rs` 中添加记忆测试模块
    - 测试 `get_memory_tree()` 接口
    - 测试 `read_memory()` 接口
    - 测试 `write_memory()` 接口
    - 测试 `search_memory()` 接口（含查询参数编码）
    - _需求: 2.1, 2.2, 2.3, 2.4, 6.1, 6.2, 6.4_

  - [ ]* 4.2 编写记忆接口属性测试
    - 在 `api_property_tests.rs` 中添加记忆测试模块
    - **属性 1: 数据序列化 Round-trip**
    - **验证需求: 2.5**
    - 验证 MemoryTreeResponse 的 round-trip
    - _需求: 2.5, 9.1, 9.2_

  - [ ]* 4.3 编写嵌套结构解析属性测试
    - **属性 9: 嵌套结构正确解析**
    - **验证需求: 9.4**
    - 验证 MemoryTreeResponse 的所有层级正确解析
    - _需求: 9.4_

  - [ ]* 4.4 编写记忆接口幂等性属性测试
    - **属性 3: 写入操作的幂等性验证**
    - **验证需求: 2.3**
    - 验证写入操作后读取反映变更
    - _需求: 2.3_

- [ ] 5. 实现任务接口集成测试
  - [ ] 5.1 实现任务接口单元测试
    - 在 `api_integration_tests.rs` 中添加任务测试模块
    - 测试 `get_jobs()` 接口
    - 测试 `get_job_detail()` 接口
    - 测试 `cancel_job()` 接口
    - 测试 `restart_job()` 接口
    - _需求: 3.1, 3.2, 3.3, 3.4, 6.1, 6.2, 6.3_

  - [ ]* 5.2 编写任务接口属性测试
    - 在 `api_property_tests.rs` 中添加任务测试模块
    - **属性 1: 数据序列化 Round-trip**
    - **验证需求: 3.5**
    - 验证 JobInfo 的 round-trip
    - _需求: 3.5, 9.1, 9.2_

  - [ ]* 5.3 编写任务接口幂等性属性测试
    - **属性 3: 写入操作的幂等性验证**
    - **验证需求: 3.3, 3.4**
    - 验证取消和重启操作后查询反映变更
    - _需求: 3.3, 3.4_

- [ ] 6. 实现日志接口集成测试
  - [ ] 6.1 实现日志接口单元测试
    - 在 `api_integration_tests.rs` 中添加日志测试模块
    - 测试 `get_logs()` 接口
    - 测试 `search_logs()` 接口（含查询参数编码）
    - 测试 `filter_logs()` 接口（含 level 和 module 参数）
    - 测试 `export_logs()` 接口
    - 测试 `clear_logs()` 接口
    - _需求: 4.1, 4.2, 4.3, 4.4, 4.5, 6.1, 6.2, 6.4_

  - [ ]* 6.2 编写日志接口属性测试
    - 在 `api_property_tests.rs` 中添加日志测试模块
    - **属性 1: 数据序列化 Round-trip**
    - **验证需求: 4.6**
    - 验证 LogEntry 的 round-trip
    - _需求: 4.6, 9.1, 9.2_

  - [ ]* 6.3 编写数组元素解析属性测试
    - **属性 10: 数组元素正确解析**
    - **验证需求: 9.5**
    - 验证日志列表的所有元素正确解析且顺序保持
    - _需求: 9.5_

- [ ] 7. 实现批准接口集成测试
  - [ ] 7.1 实现批准接口单元测试
    - 在 `api_integration_tests.rs` 中添加批准测试模块
    - 测试 `approve_operation()` 接口
    - 测试 `deny_operation()` 接口
    - _需求: 5.1, 5.2, 6.1, 6.2, 6.5_

  - [ ]* 7.2 编写批准接口属性测试
    - 在 `api_property_tests.rs` 中添加批准测试模块
    - **属性 1: 数据序列化 Round-trip**
    - **验证需求: 5.3**
    - 验证 ApprovalRequest 的 round-trip
    - _需求: 5.3, 9.1, 9.2_

- [ ] 8. 检查点 - 验证所有基础接口测试
  - 运行 `cargo test --test api_integration_tests`
  - 运行 `cargo test --test api_property_tests`
  - 确保所有测试通过，询问用户是否有问题

- [ ] 9. 实现错误处理测试
  - [ ] 9.1 实现错误处理单元测试
    - 创建 `desktop-client/tests/api_error_tests.rs`
    - 测试 4xx 错误处理（400, 401, 403, 404）
    - 测试 5xx 错误处理（500, 503）
    - 测试网络连接失败
    - 测试 JSON 解析失败
    - 测试请求超时
    - _需求: 7.1, 7.2, 7.3, 7.4, 7.5_

  - [ ]* 9.2 编写 HTTP 错误处理属性测试
    - **属性 5: HTTP 错误正确处理**
    - **验证需求: 7.1, 7.2**
    - 使用 proptest 生成随机错误状态码
    - 验证所有错误返回包含错误信息的 Error 类型
    - _需求: 7.1, 7.2_

- [ ] 10. 实现认证测试
  - [ ] 10.1 实现认证单元测试
    - 在 `api_error_tests.rs` 中添加认证测试模块
    - 测试有效令牌的请求包含 Authorization 头
    - 测试无效令牌返回 401 错误
    - 测试缺失令牌返回 401 错误
    - _需求: 8.1, 8.2, 8.3_

  - [ ]* 10.2 编写认证头传递属性测试
    - **属性 6: 认证头正确传递**
    - **验证需求: 8.1**
    - 验证所有受保护端点的请求包含正确的 Bearer token
    - _需求: 8.1_

- [ ] 11. 实现数据解析测试
  - [ ] 11.1 实现可选字段解析测试
    - 在 `api_integration_tests.rs` 中添加数据解析测试
    - 测试包含 null 值的响应正确解析为 None
    - 测试包含值的可选字段正确解析
    - _需求: 9.3_

  - [ ]* 11.2 编写可选字段属性测试
    - **属性 8: 可选字段正确处理**
    - **验证需求: 9.3**
    - 使用 proptest 生成包含/不包含可选字段的响应
    - 验证 null 值正确解析为 None
    - _需求: 9.3_

- [ ] 12. 实现端到端流程测试
  - [ ] 12.1 实现端到端流程测试
    - 创建 `desktop-client/tests/api_e2e_tests.rs`
    - 测试"创建对话 → 发送消息 → 获取历史"流程
    - 测试"获取记忆树 → 读取文件 → 写入文件"流程
    - 测试"获取任务列表 → 获取任务详情"流程
    - 测试"获取日志 → 搜索日志 → 过滤日志"流程
    - 测试"发送消息 → 批准操作"流程
    - _需求: 10.1, 10.2, 10.3, 10.4, 10.5_

- [ ] 13. 实现性能和可靠性测试
  - [ ] 13.1 实现并发测试
    - 在 `api_e2e_tests.rs` 中添加性能测试模块
    - 测试 10 个并发请求的正确处理
    - 测试大尺寸请求体的传输
    - 测试大尺寸响应的解析
    - _需求: 12.1, 12.2, 12.3_

  - [ ]* 13.2 编写并发请求属性测试
    - **属性 7: 并发请求正确处理**
    - **验证需求: 12.1**
    - 使用 proptest 生成随机数量的并发请求
    - 验证所有请求成功完成且返回正确响应
    - _需求: 12.1_

- [ ] 14. 最终检查点 - 完整测试验证
  - 运行 `cargo test` 验证所有测试
  - 运行 `cargo build` 验证编译无错误
  - 确保测试覆盖率达到目标
  - 询问用户是否有问题或需要调整

## 注意事项

- 任务标记 `*` 的为可选测试任务，可以跳过以加快 MVP 开发
- 每个任务都引用了具体的需求编号，确保可追溯性
- 检查点任务确保增量验证，及早发现问题
- 属性测试验证通用正确性属性，单元测试验证具体示例
- 所有测试使用真实的 Web Gateway 实例和内存数据库
- 遵循 TDD 方法：先写测试，再实现功能
