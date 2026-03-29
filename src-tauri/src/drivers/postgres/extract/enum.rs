use serde_json::Value as JsonValue;

#[inline(always)]
pub fn extract_or_null(buf: &[u8]) -> JsonValue {
    match std::str::from_utf8(buf) {
        Ok(s) => JsonValue::String(s.to_string()),
        Err(_) => JsonValue::Null,
    }
}
