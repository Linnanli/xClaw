//! 知识库管理 Handler
//!
//! 端点：
//! - GET    /api/knowledge-bases                        — 知识库列表（支持 ?search= 搜索）
//! - POST   /api/knowledge-bases                        — 创建知识库
//! - PUT    /api/knowledge-bases/{id}                   — 更新知识库
//! - DELETE /api/knowledge-bases/{id}                   — 删除知识库（级联删除文档）
//! - POST   /api/knowledge-bases/{id}/documents         — 上传文档（JSON 元数据）
//! - GET    /api/knowledge-bases/{id}/documents         — 文档列表
//! - DELETE /api/knowledge-bases/{id}/documents/{doc_id}— 删除文档
//! - POST   /api/knowledge-bases/{id}/search            — 检索测试（mock）

use crate::error::{Error, Result};
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

// ============================================================================
// 请求 / 查询结构体
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct KnowledgeBaseSearch {
    pub search: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateKnowledgeBaseRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateKnowledgeBaseRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub enabled: Option<bool>,
}

/// 文档上传请求（JSON 元数据，不做真实文件存储）
#[derive(Debug, Deserialize, Serialize)]
pub struct UploadDocumentRequest {
    pub filename: String,
    /// pdf / docx / md / txt
    pub file_type: String,
    #[serde(default)]
    pub file_size: i64,
}

#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    pub query: String,
}

// ============================================================================
// 端点实现
// ============================================================================

/// GET /api/knowledge-bases — 知识库列表
pub async fn list_knowledge_bases(
    State(state): State<AppState>,
    Query(params): Query<KnowledgeBaseSearch>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let rows = match params.search.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(keyword) => {
            let pattern = format!("%{}%", keyword);
            client.query(
                "SELECT id, name, description, document_count, enabled, \
                        allowed_departments, allowed_roles, created_at, updated_at \
                 FROM knowledge_bases \
                 WHERE name ILIKE $1 \
                 ORDER BY created_at DESC",
                &[&pattern],
            ).await
        }
        None => {
            client.query(
                "SELECT id, name, description, document_count, enabled, \
                        allowed_departments, allowed_roles, created_at, updated_at \
                 FROM knowledge_bases \
                 ORDER BY created_at DESC",
                &[],
            ).await
        }
    }.map_err(|e| Error::Database(e.to_string()))?;

    let data: Vec<_> = rows.iter().map(row_to_knowledge_base).collect();
    Ok(Json(json!({ "data": data, "total": data.len() })))
}

/// POST /api/knowledge-bases — 创建知识库
pub async fn create_knowledge_base(
    State(state): State<AppState>,
    Json(payload): Json<CreateKnowledgeBaseRequest>,
) -> Result<Json<serde_json::Value>> {
    if payload.name.trim().is_empty() {
        return Err(Error::Validation("知识库名称不能为空".into()));
    }

    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let row = client.query_one(
        "INSERT INTO knowledge_bases (name, description) \
         VALUES ($1, $2) \
         RETURNING id, name, description, document_count, enabled, \
                   allowed_departments, allowed_roles, created_at, updated_at",
        &[&payload.name.trim(), &payload.description],
    ).await.map_err(|e| Error::Database(e.to_string()))?;

    Ok(Json(row_to_knowledge_base(&row)))
}

/// PUT /api/knowledge-bases/{id} — 更新知识库
pub async fn update_knowledge_base(
    State(state): State<AppState>,
    Path(kb_id): Path<Uuid>,
    Json(payload): Json<UpdateKnowledgeBaseRequest>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 先确认存在
    let existing = client.query_opt(
        "SELECT id FROM knowledge_bases WHERE id = $1",
        &[&kb_id],
    ).await.map_err(|e| Error::Database(e.to_string()))?
     .ok_or_else(|| Error::NotFound("知识库不存在".into()))?;
    let _ = existing;

    // 动态构建 SET 子句，只更新提供的字段
    let mut set_parts: Vec<String> = Vec::new();
    let mut param_idx: i32 = 1;
    let mut params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>> = Vec::new();

    if let Some(ref name) = payload.name {
        if name.trim().is_empty() {
            return Err(Error::Validation("知识库名称不能为空".into()));
        }
        set_parts.push(format!("name = ${}", param_idx));
        params.push(Box::new(name.trim().to_string()));
        param_idx += 1;
    }
    if let Some(ref desc) = payload.description {
        set_parts.push(format!("description = ${}", param_idx));
        params.push(Box::new(desc.clone()));
        param_idx += 1;
    }
    if let Some(enabled) = payload.enabled {
        set_parts.push(format!("enabled = ${}", param_idx));
        params.push(Box::new(enabled));
        param_idx += 1;
    }

    if set_parts.is_empty() {
        return Err(Error::Validation("没有提供任何更新字段".into()));
    }

    set_parts.push(format!("updated_at = ${}", param_idx));
    params.push(Box::new(chrono::Utc::now()));
    param_idx += 1;

    params.push(Box::new(kb_id));
    let sql = format!(
        "UPDATE knowledge_bases SET {} WHERE id = ${} \
         RETURNING id, name, description, document_count, enabled, \
                   allowed_departments, allowed_roles, created_at, updated_at",
        set_parts.join(", "),
        param_idx
    );

    let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
        params.iter().map(|p| &**p as &(dyn tokio_postgres::types::ToSql + Sync)).collect();

    let row = client.query_one(&sql, &param_refs).await
        .map_err(|e| Error::Database(e.to_string()))?;

    Ok(Json(row_to_knowledge_base(&row)))
}

/// DELETE /api/knowledge-bases/{id} — 删除知识库（ON DELETE CASCADE 处理文档）
pub async fn delete_knowledge_base(
    State(state): State<AppState>,
    Path(kb_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let affected = client.execute(
        "DELETE FROM knowledge_bases WHERE id = $1",
        &[&kb_id],
    ).await.map_err(|e| Error::Database(e.to_string()))?;

    if affected == 0 {
        return Err(Error::NotFound("知识库不存在".into()));
    }

    Ok(Json(json!({ "message": "知识库已删除" })))
}

/// POST /api/knowledge-bases/{id}/documents — 上传文档（存元数据）
pub async fn upload_document(
    State(state): State<AppState>,
    Path(kb_id): Path<Uuid>,
    Json(payload): Json<UploadDocumentRequest>,
) -> Result<Json<serde_json::Value>> {
    validate_file_type(&payload.file_type)?;

    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 确认知识库存在
    client.query_opt("SELECT id FROM knowledge_bases WHERE id = $1", &[&kb_id])
        .await.map_err(|e| Error::Database(e.to_string()))?
        .ok_or_else(|| Error::NotFound("知识库不存在".into()))?;

    let row = client.query_one(
        "INSERT INTO kb_documents (knowledge_base_id, filename, file_type, file_size, status) \
         VALUES ($1, $2, $3, $4, 'pending') \
         RETURNING id, knowledge_base_id, filename, file_type, file_size, \
                   status, chunk_count, error_message, storage_path, uploaded_at, processed_at",
        &[&kb_id, &payload.filename, &payload.file_type, &payload.file_size],
    ).await.map_err(|e| Error::Database(e.to_string()))?;

    // 更新文档计数
    let _ = client.execute(
        "UPDATE knowledge_bases SET document_count = document_count + 1, updated_at = NOW() WHERE id = $1",
        &[&kb_id],
    ).await;

    Ok(Json(row_to_document(&row)))
}

/// GET /api/knowledge-bases/{id}/documents — 文档列表
pub async fn list_documents(
    State(state): State<AppState>,
    Path(kb_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 确认知识库存在
    client.query_opt("SELECT id FROM knowledge_bases WHERE id = $1", &[&kb_id])
        .await.map_err(|e| Error::Database(e.to_string()))?
        .ok_or_else(|| Error::NotFound("知识库不存在".into()))?;

    let rows = client.query(
        "SELECT id, knowledge_base_id, filename, file_type, file_size, \
                status, chunk_count, error_message, storage_path, uploaded_at, processed_at \
         FROM kb_documents \
         WHERE knowledge_base_id = $1 \
         ORDER BY uploaded_at DESC",
        &[&kb_id],
    ).await.map_err(|e| Error::Database(e.to_string()))?;

    let data: Vec<_> = rows.iter().map(row_to_document).collect();
    Ok(Json(json!({ "data": data, "total": data.len() })))
}

/// DELETE /api/knowledge-bases/{id}/documents/{doc_id} — 删除文档
pub async fn delete_document(
    State(state): State<AppState>,
    Path((kb_id, doc_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let affected = client.execute(
        "DELETE FROM kb_documents WHERE id = $1 AND knowledge_base_id = $2",
        &[&doc_id, &kb_id],
    ).await.map_err(|e| Error::Database(e.to_string()))?;

    if affected == 0 {
        return Err(Error::NotFound("文档不存在".into()));
    }

    // 更新文档计数（不低于 0）
    let _ = client.execute(
        "UPDATE knowledge_bases \
         SET document_count = GREATEST(document_count - 1, 0), updated_at = NOW() \
         WHERE id = $1",
        &[&kb_id],
    ).await;

    Ok(Json(json!({ "message": "文档已删除" })))
}

/// POST /api/knowledge-bases/{id}/search — 检索测试
///
/// TODO: 向量存储方案待实现。
/// 当前为 stub，返回空结果。后续需要：
/// 1. 集成向量数据库（推荐 pgvector 扩展，直接在 PostgreSQL 中存储向量）
/// 2. 文档上传时触发切片 + 向量化（可用 ironclaw 的 document_extraction 模块）
/// 3. 检索时执行 ANN 相似度查询（cosine similarity）
/// 参考：https://github.com/pgvector/pgvector
pub async fn search_knowledge_base(
    State(state): State<AppState>,
    Path(kb_id): Path<Uuid>,
    Json(_payload): Json<SearchRequest>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 确认知识库存在
    client.query_opt("SELECT id FROM knowledge_bases WHERE id = $1", &[&kb_id])
        .await.map_err(|e| Error::Database(e.to_string()))?
        .ok_or_else(|| Error::NotFound("知识库不存在".into()))?;

    Ok(Json(json!({
        "results": [],
        "message": "向量化引擎未配置，请先完成文档处理"
    })))
}

// ============================================================================
// 内部辅助函数
// ============================================================================

fn validate_file_type(file_type: &str) -> Result<()> {
    match file_type {
        "pdf" | "docx" | "md" | "txt" => Ok(()),
        _ => Err(Error::Validation(format!(
            "不支持的文件类型 '{}'，仅支持 pdf/docx/md/txt", file_type
        ))),
    }
}

fn row_to_knowledge_base(row: &tokio_postgres::Row) -> serde_json::Value {
    json!({
        "id":                   row.get::<_, Uuid>(0),
        "name":                 row.get::<_, String>(1),
        "description":          row.get::<_, Option<String>>(2),
        "document_count":       row.get::<_, i32>(3),
        "enabled":              row.get::<_, bool>(4),
        "allowed_departments":  row.get::<_, serde_json::Value>(5),
        "allowed_roles":        row.get::<_, serde_json::Value>(6),
        "created_at":           row.get::<_, chrono::DateTime<chrono::Utc>>(7),
        "updated_at":           row.get::<_, chrono::DateTime<chrono::Utc>>(8),
    })
}

fn row_to_document(row: &tokio_postgres::Row) -> serde_json::Value {
    json!({
        "id":                 row.get::<_, Uuid>(0),
        "knowledge_base_id":  row.get::<_, Uuid>(1),
        "filename":           row.get::<_, String>(2),
        "file_type":          row.get::<_, String>(3),
        "file_size":          row.get::<_, i64>(4),
        "status":             row.get::<_, String>(5),
        "chunk_count":        row.get::<_, i32>(6),
        "error_message":      row.get::<_, Option<String>>(7),
        "storage_path":       row.get::<_, Option<String>>(8),
        "uploaded_at":        row.get::<_, chrono::DateTime<chrono::Utc>>(9),
        "processed_at":       row.get::<_, Option<chrono::DateTime<chrono::Utc>>>(10),
    })
}
