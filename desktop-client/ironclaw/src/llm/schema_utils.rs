//! JSON Schema 规范化工具。
//!
//! 为 OpenAI "strict mode" 工具调用（[docs](https://platform.openai.com/docs/guides/function-calling#strict-mode)）
//! 准备输入 schema：递归地把所有对象标记为 `additionalProperties: false`，
//! 把所有 property 补进 `required`，并把原本非必填的字段转成 nullable 类型。
//!
//! 原位于 `rig_adapter.rs`，Phase 2 Step I 从 rig 体系解耦搬到此处，
//! 供 `openai_codex_provider` 等仍在用 Responses API strict 工具调用的
//! 路径继续使用。

use serde_json::Value as JsonValue;

/// Normalize a JSON schema for OpenAI strict mode function calling.
///
/// - All `type: object` schemas get `additionalProperties: false`
/// - `"required"` must list ALL property keys
/// - Optional fields use `"type": ["<original>", "null"]` instead of being omitted from `required`
/// - Nested objects and array items are recursively normalized
///
/// This is applied as a clone-and-transform at the provider boundary so the
/// original tool definitions remain unchanged for other providers.
pub(crate) fn normalize_schema_strict(schema: &JsonValue) -> JsonValue {
    let mut schema = schema.clone();
    normalize_schema_recursive(&mut schema);
    schema
}

fn normalize_schema_recursive(schema: &mut JsonValue) {
    let obj = match schema.as_object_mut() {
        Some(o) => o,
        None => return,
    };

    // Recurse into combinators: anyOf, oneOf, allOf
    for key in &["anyOf", "oneOf", "allOf"] {
        if let Some(JsonValue::Array(variants)) = obj.get_mut(*key) {
            for variant in variants.iter_mut() {
                normalize_schema_recursive(variant);
            }
        }
    }

    // Recurse into array items
    if let Some(items) = obj.get_mut("items") {
        normalize_schema_recursive(items);
    }

    // Recurse into `not`, `if`, `then`, `else`
    for key in &["not", "if", "then", "else"] {
        if let Some(sub) = obj.get_mut(*key) {
            normalize_schema_recursive(sub);
        }
    }

    // Only apply object-level normalization if this schema has "properties"
    // (explicit object schema) or type == "object"
    let is_object = obj
        .get("type")
        .and_then(|t| t.as_str())
        .map(|t| t == "object")
        .unwrap_or(false);
    let has_properties = obj.contains_key("properties");

    if !is_object && !has_properties {
        return;
    }

    // Ensure "type": "object" is present
    if !obj.contains_key("type") && has_properties {
        obj.insert("type".to_string(), JsonValue::String("object".to_string()));
    }

    // Force additionalProperties: false (overwrite any existing value)
    obj.insert("additionalProperties".to_string(), JsonValue::Bool(false));

    // Ensure "properties" exists
    if !obj.contains_key("properties") {
        obj.insert(
            "properties".to_string(),
            JsonValue::Object(serde_json::Map::new()),
        );
    }

    // Collect current required set
    let current_required: std::collections::HashSet<String> = obj
        .get("required")
        .and_then(|r| r.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // Get all property keys (sorted for deterministic output)
    let all_keys: Vec<String> = obj
        .get("properties")
        .and_then(|p| p.as_object())
        .map(|props| {
            let mut keys: Vec<String> = props.keys().cloned().collect();
            keys.sort();
            keys
        })
        .unwrap_or_default();

    // For properties NOT in the original required list, make them nullable
    if let Some(JsonValue::Object(props)) = obj.get_mut("properties") {
        for key in &all_keys {
            // Recurse into each property's schema FIRST (before make_nullable,
            // which may change the type to an array and prevent object detection)
            if let Some(prop_schema) = props.get_mut(key) {
                normalize_schema_recursive(prop_schema);
            }
            // Then make originally-optional properties nullable
            if !current_required.contains(key)
                && let Some(prop_schema) = props.get_mut(key)
            {
                make_nullable(prop_schema);
            }
        }
    }

    // Set required to ALL property keys
    let required_value: Vec<JsonValue> = all_keys.into_iter().map(JsonValue::String).collect();
    obj.insert("required".to_string(), JsonValue::Array(required_value));
}

/// Make a property schema nullable for OpenAI strict mode.
///
/// If it has a simple `"type": "<T>"`, converts to `"type": ["<T>", "null"]`.
/// If it already has an array type, adds "null" if not present.
/// Otherwise, wraps with `anyOf: [<existing>, {"type": "null"}]`.
fn make_nullable(schema: &mut JsonValue) {
    let obj = match schema.as_object_mut() {
        Some(o) => o,
        None => return,
    };

    if let Some(type_val) = obj.get("type").cloned() {
        match type_val {
            // "type": "string" → "type": ["string", "null"]
            JsonValue::String(ref t) if t != "null" => {
                obj.insert("type".to_string(), serde_json::json!([t, "null"]));
            }
            // "type": ["string", "integer"] → add "null" if missing
            JsonValue::Array(ref arr) => {
                let has_null = arr.iter().any(|v| v.as_str() == Some("null"));
                if !has_null {
                    let mut new_arr = arr.clone();
                    new_arr.push(JsonValue::String("null".to_string()));
                    obj.insert("type".to_string(), JsonValue::Array(new_arr));
                }
            }
            _ => {}
        }
    } else {
        // No "type" key — wrap with anyOf including null
        // (handles enum-only, $ref, or combinator schemas)
        let existing = JsonValue::Object(obj.clone());
        obj.clear();
        obj.insert(
            "anyOf".to_string(),
            serde_json::json!([existing, {"type": "null"}]),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn simple_object_gets_additional_properties_false_and_all_required() {
        let input = json!({
            "type": "object",
            "properties": {
                "a": { "type": "string" },
                "b": { "type": "integer" }
            },
            "required": ["a"]
        });
        let out = normalize_schema_strict(&input);
        assert_eq!(out["additionalProperties"], json!(false));
        assert_eq!(out["required"], json!(["a", "b"]));
        // 未在原 required 的字段应被标记为 nullable
        assert_eq!(out["properties"]["b"]["type"], json!(["integer", "null"]));
    }

    #[test]
    fn implicit_object_with_properties_infers_type() {
        let input = json!({
            "properties": { "x": { "type": "string" } }
        });
        let out = normalize_schema_strict(&input);
        assert_eq!(out["type"], json!("object"));
        assert_eq!(out["additionalProperties"], json!(false));
    }

    #[test]
    fn nested_object_normalized_recursively() {
        let input = json!({
            "type": "object",
            "properties": {
                "child": {
                    "type": "object",
                    "properties": { "k": { "type": "string" } },
                    "required": ["k"]
                }
            },
            "required": ["child"]
        });
        let out = normalize_schema_strict(&input);
        assert_eq!(
            out["properties"]["child"]["additionalProperties"],
            json!(false)
        );
        assert_eq!(out["properties"]["child"]["required"], json!(["k"]));
    }
}
