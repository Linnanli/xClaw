use serde_json::json;

use crate::state::AppState;

pub async fn persist_disabled_items(
    state: &AppState,
    setting_key: &str,
    disabled: Vec<String>,
    subject: &str,
) -> Result<(), String> {
    let db = state.db.as_ref().ok_or("Database not available")?;
    let value = json!(disabled);
    db.set_setting(&state.owner_id, setting_key, &value)
        .await
        .map_err(|e| format!("Failed to persist disabled {}: {}", subject, e))
}
