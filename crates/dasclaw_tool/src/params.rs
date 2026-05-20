//! Parameter helpers and JSON-Schema validation for tool implementations.
//!
//! Verbatim port from `desktop-client/ironclaw/src/tools/tool.rs` per
//! ADR-129 §1.3 (F3.3 phase 2 PR, #670). The strict schema validator
//! referenced in the docs below still lives in ironclaw as
//! `tools::schema_validator::validate_strict_schema`; only the lenient
//! validator and the `require_*` / `redact_params` helpers move here.

use crate::ToolError;

/// Extract a required string parameter from a JSON object.
///
/// Returns `ToolError::InvalidParameters` if the key is missing or not a string.
pub fn require_str<'a>(params: &'a serde_json::Value, name: &str) -> Result<&'a str, ToolError> {
    params
        .get(name)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::InvalidParameters(format!("missing '{}' parameter", name)))
}

/// Extract a required parameter of any type from a JSON object.
///
/// Returns `ToolError::InvalidParameters` if the key is missing.
pub fn require_param<'a>(
    params: &'a serde_json::Value,
    name: &str,
) -> Result<&'a serde_json::Value, ToolError> {
    params
        .get(name)
        .ok_or_else(|| ToolError::InvalidParameters(format!("missing '{}' parameter", name)))
}

/// Replace sensitive parameter values with `"[REDACTED]"`.
///
/// Returns a new JSON value with the specified keys replaced. Non-object params
/// and unknown keys are passed through unchanged. The original value is cloned
/// only if there are sensitive params to redact; otherwise it is cloned once
/// (cheap — callers own the result).
///
/// Used by the agent framework before logging, hook dispatch, approval display,
/// and `ActionRecord` storage so plaintext secrets never reach those paths.
pub fn redact_params(params: &serde_json::Value, sensitive: &[&str]) -> serde_json::Value {
    if sensitive.is_empty() {
        return params.clone();
    }
    let mut redacted = params.clone();
    if let Some(obj) = redacted.as_object_mut() {
        for key in sensitive {
            if obj.contains_key(*key) {
                obj.insert(
                    (*key).to_string(),
                    serde_json::Value::String("[REDACTED]".into()),
                );
            }
        }
    }
    redacted
}

/// Lenient runtime validation of a tool's `parameters_schema()`.
///
/// Use this function at tool-registration time to catch structural mistakes
/// (missing `"type": "object"`, orphan `"required"` keys, arrays without
/// `"items"`) without rejecting intentional freeform properties.
///
/// For the stricter variant that also enforces `additionalProperties: false`,
/// enum-type consistency, and per-property `"type"` fields, see
/// `validate_strict_schema` in ironclaw's `tools::schema_validator` module
/// (used in CI tests).
///
/// Returns a list of validation errors. An empty list means the schema is valid.
///
/// # Rules enforced
///
/// 1. Top-level must have `"type": "object"`
/// 2. Top-level must have `"properties"` as an object
/// 3. Every key in `"required"` must exist in `"properties"`
/// 4. Nested objects follow the same rules recursively
/// 5. Array properties should have `"items"` defined
///
/// Properties without a `"type"` field are allowed (freeform/any-type).
/// This is an intentional pattern used by tools like `json` and `http` for
/// OpenAI compatibility, since union types with arrays require `items`.
/// Maximum nesting depth for tool schema validation to prevent stack overflow
/// on maliciously crafted schemas.
const MAX_SCHEMA_DEPTH: usize = 16;

/// Returns true if the schema uses `oneOf`, `anyOf`, or `allOf` combinators
/// where at least one variant is an object type (has `type: "object"` or `properties`).
fn has_object_combinator_variants(schema: &serde_json::Value) -> bool {
    for key in ["oneOf", "anyOf", "allOf"] {
        if let Some(variants) = schema.get(key).and_then(|v| v.as_array())
            && variants.iter().any(|v| {
                v.get("type").and_then(|t| t.as_str()) == Some("object")
                    || v.get("properties").is_some()
            })
        {
            return true;
        }
    }
    false
}

pub fn validate_tool_schema(schema: &serde_json::Value, path: &str) -> Vec<String> {
    validate_tool_schema_inner(schema, path, 0)
}

fn validate_tool_schema_inner(schema: &serde_json::Value, path: &str, depth: usize) -> Vec<String> {
    let mut errors = Vec::new();

    if depth > MAX_SCHEMA_DEPTH {
        errors.push(format!(
            "{path}: schema nesting exceeds maximum depth of {MAX_SCHEMA_DEPTH}"
        ));
        return errors;
    }

    // Report non-array combinator values as errors.
    for key in ["oneOf", "anyOf", "allOf"] {
        if let Some(val) = schema.get(key)
            && !val.is_array()
        {
            errors.push(format!("{path}: \"{key}\" must be an array"));
        }
    }

    let has_combinators = has_object_combinator_variants(schema);

    // Rule 1: must have "type": "object" at this level (unless combinators define the structure)
    match schema.get("type").and_then(|t| t.as_str()) {
        Some("object") => {}
        Some(other) => {
            errors.push(format!("{path}: expected type \"object\", got \"{other}\""));
            return errors; // Can't check further
        }
        None => {
            if !has_combinators {
                errors.push(format!("{path}: missing \"type\": \"object\""));
                return errors;
            }
        }
    }

    // Validate combinator variants recursively
    for key in ["allOf", "oneOf", "anyOf"] {
        if let Some(variants) = schema.get(key).and_then(|v| v.as_array()) {
            for (i, variant) in variants.iter().enumerate() {
                if variant.get("type").and_then(|t| t.as_str()) == Some("object")
                    || variant.get("properties").is_some()
                {
                    let variant_path = format!("{path}.{key}[{i}]");
                    errors.extend(validate_tool_schema_inner(
                        variant,
                        &variant_path,
                        depth + 1,
                    ));
                }
            }
        }
    }

    // Rule 2: must have "properties" as an object (unless combinators define them)
    let properties = match schema.get("properties").and_then(|p| p.as_object()) {
        Some(p) => p,
        None => {
            if !has_combinators {
                errors.push(format!("{path}: missing or non-object \"properties\""));
                return errors;
            }
            // Combinators define the structure — validate top-level `required` keys
            // against merged properties from all combinator variants.
            if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
                let mut merged_keys = std::collections::HashSet::new();
                if let Some(all_of) = schema.get("allOf").and_then(|a| a.as_array()) {
                    for variant in all_of {
                        if let Some(props) = variant.get("properties").and_then(|p| p.as_object()) {
                            merged_keys.extend(props.keys().cloned());
                        }
                    }
                }
                for key in ["oneOf", "anyOf"] {
                    if let Some(variants) = schema.get(key).and_then(|v| v.as_array()) {
                        for variant in variants {
                            if let Some(props) =
                                variant.get("properties").and_then(|p| p.as_object())
                            {
                                merged_keys.extend(props.keys().cloned());
                            }
                        }
                    }
                }
                for req in required {
                    if let Some(key) = req.as_str()
                        && !merged_keys.contains(key)
                    {
                        errors.push(format!(
                            "{path}: required key \"{key}\" not found in any combinator variant properties"
                        ));
                    }
                }
            }
            return errors;
        }
    };

    // Rule 3: every key in "required" must exist in "properties"
    if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
        for req in required {
            if let Some(key) = req.as_str()
                && !properties.contains_key(key)
            {
                errors.push(format!(
                    "{path}: required key \"{key}\" not found in properties"
                ));
            }
        }
    }

    // Rule 4 & 5: recurse into nested objects and check arrays
    for (key, prop) in properties {
        let prop_path = format!("{path}.{key}");
        if let Some(prop_type) = prop.get("type").and_then(|t| t.as_str()) {
            match prop_type {
                "object" => {
                    errors.extend(validate_tool_schema_inner(prop, &prop_path, depth + 1));
                }
                "array" => {
                    if let Some(items) = prop.get("items") {
                        // If items is an object type, recurse
                        if items.get("type").and_then(|t| t.as_str()) == Some("object") {
                            errors.extend(validate_tool_schema_inner(
                                items,
                                &format!("{prop_path}.items"),
                                depth + 1,
                            ));
                        }
                    } else {
                        errors.push(format!("{prop_path}: array property missing \"items\""));
                    }
                }
                _ => {}
            }
        }
        // No "type" field is intentionally allowed (freeform properties)
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test secret value used in `redact_params` tests; mirrors
    /// `ironclaw::testing::credentials::TEST_REDACT_SECRET` so the assertions
    /// stay byte-for-byte identical to the original tests.
    const TEST_REDACT_SECRET: &str = "sk-secret";

    #[test]
    fn test_require_str_present() {
        let params = serde_json::json!({"name": "alice"});
        assert_eq!(require_str(&params, "name").unwrap(), "alice");
    }

    #[test]
    fn test_require_str_missing() {
        let params = serde_json::json!({});
        let err = require_str(&params, "name").unwrap_err();
        assert!(err.to_string().contains("missing 'name'"));
    }

    #[test]
    fn test_require_str_wrong_type() {
        let params = serde_json::json!({"name": 42});
        let err = require_str(&params, "name").unwrap_err();
        assert!(err.to_string().contains("missing 'name'"));
    }

    #[test]
    fn test_require_param_present() {
        let params = serde_json::json!({"data": [1, 2, 3]});
        assert_eq!(
            require_param(&params, "data").unwrap(),
            &serde_json::json!([1, 2, 3])
        );
    }

    #[test]
    fn test_require_param_missing() {
        let params = serde_json::json!({});
        let err = require_param(&params, "data").unwrap_err();
        assert!(err.to_string().contains("missing 'data'"));
    }

    #[test]
    fn test_redact_params_replaces_sensitive_key() {
        let params = serde_json::json!({"name": "openai_key", "value": TEST_REDACT_SECRET});
        let redacted = redact_params(&params, &["value"]);
        assert_eq!(redacted["name"], "openai_key");
        assert_eq!(redacted["value"], "[REDACTED]");
        // Original unchanged
        assert_eq!(params["value"], TEST_REDACT_SECRET);
    }

    #[test]
    fn test_redact_params_empty_sensitive_is_noop() {
        let params = serde_json::json!({"name": "key", "value": "secret"});
        let redacted = redact_params(&params, &[]);
        assert_eq!(redacted, params);
    }

    #[test]
    fn test_redact_params_missing_key_is_noop() {
        let params = serde_json::json!({"name": "key"});
        let redacted = redact_params(&params, &["value"]);
        assert_eq!(redacted, params);
    }

    #[test]
    fn test_redact_params_non_object_is_passthrough() {
        let params = serde_json::json!("just a string");
        let redacted = redact_params(&params, &["value"]);
        assert_eq!(redacted, params);
    }

    #[test]
    fn test_validate_schema_valid() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "A name" }
            },
            "required": ["name"]
        });
        let errors = validate_tool_schema(&schema, "test");
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    }

    #[test]
    fn test_validate_schema_missing_type() {
        let schema = serde_json::json!({
            "properties": {
                "name": { "type": "string" }
            }
        });
        let errors = validate_tool_schema(&schema, "test");
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("missing \"type\": \"object\""));
    }

    #[test]
    fn test_validate_schema_wrong_type() {
        let schema = serde_json::json!({
            "type": "string"
        });
        let errors = validate_tool_schema(&schema, "test");
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("expected type \"object\""));
    }

    #[test]
    fn test_validate_schema_required_not_in_properties() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            },
            "required": ["name", "age"]
        });
        let errors = validate_tool_schema(&schema, "test");
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("\"age\" not found in properties"));
    }

    #[test]
    fn test_validate_schema_nested_object() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "config": {
                    "type": "object",
                    "properties": {
                        "key": { "type": "string" }
                    },
                    "required": ["key", "missing"]
                }
            }
        });
        let errors = validate_tool_schema(&schema, "test");
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("test.config"));
        assert!(errors[0].contains("\"missing\" not found"));
    }

    #[test]
    fn test_validate_schema_array_missing_items() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "tags": { "type": "array", "description": "Tags" }
            }
        });
        let errors = validate_tool_schema(&schema, "test");
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("array property missing \"items\""));
    }

    #[test]
    fn test_validate_schema_array_with_items_ok() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "tags": {
                    "type": "array",
                    "items": { "type": "string" }
                }
            }
        });
        let errors = validate_tool_schema(&schema, "test");
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    }

    #[test]
    fn test_validate_schema_freeform_property_allowed() {
        // Properties without "type" are intentionally allowed (json/http tools)
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "data": { "description": "Any JSON value" }
            },
            "required": ["data"]
        });
        let errors = validate_tool_schema(&schema, "test");
        assert!(
            errors.is_empty(),
            "freeform property should be allowed: {errors:?}"
        );
    }

    #[test]
    fn test_validate_schema_nested_array_items_object() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "headers": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string" },
                            "value": { "type": "string" }
                        },
                        "required": ["name", "value"]
                    }
                }
            }
        });
        let errors = validate_tool_schema(&schema, "test");
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    }

    #[test]
    fn test_validate_schema_nested_array_items_object_bad() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "headers": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string" }
                        },
                        "required": ["name", "missing_field"]
                    }
                }
            }
        });
        let errors = validate_tool_schema(&schema, "test");
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("headers.items"));
        assert!(errors[0].contains("\"missing_field\""));
    }

    /// Regression test for issue #975: deeply nested schemas must not cause
    /// stack overflow. The validator should stop at MAX_SCHEMA_DEPTH and
    /// report an error instead of recursing infinitely.
    #[test]
    fn test_validate_schema_depth_limit() {
        // Build a schema nested 20 levels deep (exceeds MAX_SCHEMA_DEPTH=16)
        let mut schema = serde_json::json!({
            "type": "object",
            "properties": {
                "leaf": { "type": "string" }
            }
        });
        for _ in 0..20 {
            schema = serde_json::json!({
                "type": "object",
                "properties": {
                    "nested": schema
                }
            });
        }
        let errors = validate_tool_schema(&schema, "test");
        assert!(
            errors.iter().any(|e| e.contains("maximum depth")),
            "expected depth limit error, got: {errors:?}"
        );
    }
}
