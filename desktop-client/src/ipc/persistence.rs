use serde_json::json;
use std::collections::HashSet;

use crate::state::AppState;

pub async fn persist_disabled_items(
    state: &AppState,
    setting_key: &str,
    disabled: Vec<String>,
    subject: &str,
) -> Result<(), String> {
    let db = state.db.as_ref().ok_or("Database not available")?;
    let value = json!(disabled);
    db.set_setting(&state.scope_id, setting_key, &value)
        .await
        .map_err(|e| format!("Failed to persist disabled {}: {}", subject, e))
}

pub async fn load_string_set(state: &AppState, setting_key: &str) -> Result<HashSet<String>, String> {
    let db = state.db.as_ref().ok_or("Database not available")?;
    let value = db
        .get_setting(&state.scope_id, setting_key)
        .await
        .map_err(|e| format!("Failed to load setting {}: {}", setting_key, e))?;

    let Some(value) = value else {
        return Ok(HashSet::new());
    };

    let names = serde_json::from_value::<Vec<String>>(value)
        .map_err(|e| format!("Invalid payload for {}: {}", setting_key, e))?;
    Ok(names.into_iter().collect())
}
