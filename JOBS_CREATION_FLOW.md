# 客户端任务创建流程

## 概述

在 Desktop Client 中，任务（Jobs）不是直接由用户创建的，而是通过以下方式产生的：

1. **聊天消息触发** - 用户在聊天中发送消息
2. **Agent 处理** - 后端 Agent 处理消息并可能创建任务
3. **任务执行** - 任务在后端执行，前端通过 API 查看任务列表

## 详细流程

### 1. 用户发送聊天消息

**前端代码**：`desktop-client/src-ui/src/app/hooks/useAiChat.ts`

```typescript
// 用户在聊天输入框输入消息并发送
const handleSendMessage = async (content: string) => {
  const response = await fetch(`${apiUrl}/api/chat/send`, {
    method: 'POST',
    headers: {
      'Authorization': `Bearer ${authToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      content,
      thread_id: threadId,
    }),
  });
  
  const data = await response.json();
  // 返回 message_id 和 status: "accepted"
};
```

**后端处理**：`src/channels/web/handlers/chat.rs`

```rust
pub async fn chat_send_handler(
    State(state): State<Arc<GatewayState>>,
    Json(req): Json<SendMessageRequest>,
) -> Result<(StatusCode, Json<SendMessageResponse>), (StatusCode, String)> {
    // 1. 创建 IncomingMessage
    let mut msg = IncomingMessage::new("gateway", &state.user_id, &req.content);
    
    // 2. 发送到消息队列
    let tx = state.msg_tx.read().await;
    tx.send(msg).await?;
    
    // 3. 返回 202 Accepted
    Ok((
        StatusCode::ACCEPTED,
        Json(SendMessageResponse {
            message_id: msg_id,
            status: "accepted",
        }),
    ))
}
```

### 2. Agent 处理消息

**Agent 主循环**：`src/agent/agent_loop.rs`

```rust
async fn handle_message(&self, message: &IncomingMessage) -> Result<Option<String>, Error> {
    // 1. 解析消息类型
    let submission = SubmissionParser::parse(&message.content);
    
    // 2. 根据类型处理
    match submission {
        Submission::UserInput { content } => {
            // 调用 process_user_input
            self.process_user_input(message, session, thread_id, &content).await
        }
        Submission::SystemCommand { command, args } => {
            // 处理系统命令（如 /job, /status）
            self.handle_system_command(command, args).await
        }
        // ...
    }
}
```

**处理用户输入**：`src/agent/thread_ops.rs`

```rust
pub(super) async fn process_user_input(
    &self,
    message: &IncomingMessage,
    session: Arc<Mutex<Session>>,
    thread_id: Uuid,
    content: &str,
) -> Result<SubmissionResult, Error> {
    // 1. 验证线程状态
    // 2. 安全检查（DLP、秘密扫描）
    // 3. 启动 turn
    let turn_messages = {
        let mut sess = session.lock().await;
        let thread = sess.threads.get_mut(&thread_id)?;
        let turn = thread.start_turn(effective_content);
        thread.messages()
    };
    
    // 4. 运行 agentic loop（LLM 调用 + 工具执行）
    let result = self
        .run_agentic_loop(message, session.clone(), thread_id, turn_messages)
        .await;
    
    // 5. 完成 turn
    thread.complete_turn(&response);
}
```

### 3. Agent 工具调用创建任务

**工具调用**：`src/agent/dispatcher.rs`

Agent 可以通过工具调用创建任务。可用的工具包括：

```rust
// 工具列表
pub enum ToolName {
    CreateJob,      // 创建新任务
    ListJobs,       // 列出任务
    GetJobDetail,   // 获取任务详情
    CancelJob,      // 取消任务
    RestartJob,     // 重启任务
    // ...
}
```

**创建任务工具**：`src/agent/commands.rs`

```rust
pub async fn create_job(
    &self,
    user_id: &str,
    title: &str,
    description: &str,
) -> Result<Uuid, JobError> {
    // 通过 scheduler 创建任务
    let job_id = self
        .scheduler
        .dispatch_job(user_id, &title, &description, None)
        .await?;
    
    Ok(job_id)
}
```

**Scheduler 创建任务**：`src/agent/scheduler.rs`

```rust
pub async fn dispatch_job(
    &self,
    user_id: &str,
    title: &str,
    description: &str,
    metadata: Option<serde_json::Value>,
) -> Result<Uuid, JobError> {
    // 1. 创建任务 ID
    let job_id = self
        .context_manager
        .create_job_for_user(user_id, title, description)
        .await?;
    
    // 2. 设置令牌预算
    // 3. 启动 worker 任务
    // 4. 返回任务 ID
    Ok(job_id)
}
```

### 4. 前端查看任务列表

**前端代码**：`desktop-client/src-ui/src/app/components/tabs/JobsTab.tsx`

```typescript
export function JobsTab() {
  const [jobs, setJobs] = useState<JobInfo[]>([]);
  
  const fetchJobs = async () => {
    try {
      // 调用 Tauri 命令
      const jobList = await jobApi.getJobs();
      setJobs(jobList);
    } catch (err) {
      console.error('Failed to fetch jobs:', err);
    }
  };
  
  useEffect(() => {
    fetchJobs();
  }, []);
  
  // 显示任务列表
  return (
    <div>
      {jobs.map((job) => (
        <div key={job.id}>
          <h3>{job.title}</h3>
          <p>状态: {job.status}</p>
          <p>创建时间: {new Date(job.created_at).toLocaleString()}</p>
        </div>
      ))}
    </div>
  );
}
```

**Tauri 命令**：`desktop-client/src/commands.rs`

```rust
#[tauri::command]
pub async fn get_jobs(
    state: tauri::State<'_, CommandState>,
) -> Result<Vec<crate::api_client::JobInfo>> {
    state.api_client.get_jobs().await
}
```

**API 客户端**：`desktop-client/src/api_client.rs`

```rust
pub async fn get_jobs(&self) -> Result<Vec<JobInfo>> {
    let url = format!("{}/api/jobs", self.base_url);
    let response = self.client
        .get(&url)
        .header("Authorization", self.auth_header())
        .send()
        .await?
        .json::<JobListResponse>()
        .await?;
    
    Ok(response.jobs)
}
```

**后端 API**：`src/channels/web/handlers/jobs.rs`

```rust
pub async fn jobs_list_handler(
    State(state): State<Arc<GatewayState>>,
) -> Result<Json<JobListResponse>, (StatusCode, String)> {
    let store = state.store.as_ref().ok_or(...)?;
    
    // 1. 从数据库获取 sandbox 任务
    let sandbox_jobs = store.list_sandbox_jobs().await?;
    
    // 2. 从数据库获取 agent 任务
    let agent_jobs = store.list_agent_jobs().await?;
    
    // 3. 合并并排序
    let mut jobs = Vec::new();
    jobs.extend(sandbox_jobs);
    jobs.extend(agent_jobs);
    jobs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    
    // 4. 返回
    Ok(Json(JobListResponse { jobs }))
}
```

## 任务创建的触发方式

### 1. 通过 Agent 工具调用

用户在聊天中请求创建任务：

```
用户: "创建一个任务来分析日志文件"
↓
Agent 理解意图
↓
Agent 调用 create_job 工具
↓
任务被创建并执行
```

### 2. 通过系统命令

用户使用系统命令：

```
用户: "/job create 分析日志 分析系统日志文件"
↓
Agent 解析命令
↓
Agent 调用 create_job 工具
↓
任务被创建
```

### 3. 通过日程（Routine）

日程触发时自动创建任务：

```
日程触发事件
↓
Routine Engine 调用 dispatch_job
↓
任务被创建并执行
```

### 4. 通过任务重启

用户重启已完成的任务：

```
用户点击"重启任务"按钮
↓
前端调用 restart_job API
↓
后端创建新任务
↓
新任务被创建并执行
```

## 任务状态流转

```
创建 (pending)
  ↓
运行 (in_progress)
  ↓
完成 (completed) 或 失败 (failed) 或 卡住 (stuck)
  ↓
可以重启 (restart)
```

## 前端任务管理功能

### JobsTab 组件功能

1. **任务列表** - 显示所有任务
2. **状态过滤** - 按状态过滤任务（待处理、进行中、已完成、失败、卡住）
3. **任务详情** - 查看任务详细信息
4. **任务操作**：
   - 取消任务（进行中或待处理）
   - 重启任务（已完成、失败或卡住）
5. **刷新** - 手动刷新任务列表

### 任务详情包含

- 任务 ID
- 任务标题
- 任务描述
- 任务状态
- 创建时间
- 开始时间（如果已开始）
- 完成时间（如果已完成）
- 任务事件列表

## 相关文件

### 前端文件

- `desktop-client/src-ui/src/app/components/tabs/JobsTab.tsx` - 任务标签页组件
- `desktop-client/src-ui/src/app/utils/tauri.ts` - Tauri API 定义
- `desktop-client/src/commands.rs` - Tauri 命令实现
- `desktop-client/src/api_client.rs` - API 客户端

### 后端文件

- `src/channels/web/handlers/jobs.rs` - Jobs API 处理器
- `src/channels/web/handlers/chat.rs` - Chat API 处理器
- `src/agent/agent_loop.rs` - Agent 主循环
- `src/agent/thread_ops.rs` - 线程操作
- `src/agent/commands.rs` - Agent 命令（包括创建任务）
- `src/agent/scheduler.rs` - 任务调度器
- `src/agent/routine_engine.rs` - 日程引擎

## 总结

客户端任务的创建流程：

1. **用户交互** - 用户在聊天中发送消息或使用命令
2. **消息处理** - 后端 Agent 处理消息
3. **工具调用** - Agent 根据需要调用创建任务工具
4. **任务执行** - 任务在后端执行
5. **前端查看** - 用户在 JobsTab 中查看任务列表和详情

任务不是由前端直接创建的，而是由后端 Agent 根据用户的请求或日程触发创建的。前端的作用是显示任务列表、查看任务详情、以及执行任务操作（取消、重启）。
