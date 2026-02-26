//! RFC 8785-style JSON Canonicalization.
//!
//! Produces deterministic JSON output for signing:
//! - Object keys sorted lexicographically
//! - No whitespace
//! - Numbers serialized consistently
//! - No trailing commas
//!
//! This ensures identical content produces identical signatures
//! regardless of serialization library or platform.

use serde_json::Value;

use crate::error::CoreError;

/// Canonicalize a JSON value to a deterministic byte string.
/// Keys are sorted lexicographically at every nesting level.
pub fn canonicalize(value: &Value) -> Result<Vec<u8>, CoreError> {
    let mut buf = Vec::new();
    write_canonical(value, &mut buf)?;
    Ok(buf)
}

/// Canonicalize a serializable Rust value.
pub fn canonicalize_value<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, CoreError> {
    let json = serde_json::to_value(value).map_err(CoreError::Serialization)?;
    canonicalize(&json)
}

fn write_canonical(value: &Value, buf: &mut Vec<u8>) -> Result<(), CoreError> {
    match value {
        Value::Null => buf.extend_from_slice(b"null"),
        Value::Bool(b) => {
            buf.extend_from_slice(if *b { b"true" } else { b"false" });
        }
        Value::Number(n) => {
            // Integers: no decimal point. Floats: standard representation.
            let s = if let Some(i) = n.as_i64() {
                i.to_string()
            } else if let Some(u) = n.as_u64() {
                u.to_string()
            } else if let Some(f) = n.as_f64() {
                format_float(f)
            } else {
                return Err(CoreError::Canonicalization(
                    "unsupported number type".into(),
                ));
            };
            buf.extend_from_slice(s.as_bytes());
        }
        Value::String(s) => {
            write_canonical_string(s, buf);
        }
        Value::Array(arr) => {
            buf.push(b'[');
            for (i, item) in arr.iter().enumerate() {
                if i > 0 {
                    buf.push(b',');
                }
                write_canonical(item, buf)?;
            }
            buf.push(b']');
        }
        Value::Object(map) => {
            // Sort keys lexicographically
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();

            buf.push(b'{');
            for (i, key) in keys.iter().enumerate() {
                if i > 0 {
                    buf.push(b',');
                }
                write_canonical_string(key, buf);
                buf.push(b':');
                write_canonical(map.get(*key).unwrap(), buf)?;
            }
            buf.push(b'}');
        }
    }
    Ok(())
}

fn write_canonical_string(s: &str, buf: &mut Vec<u8>) {
    buf.push(b'"');
    for ch in s.chars() {
        match ch {
            '"' => buf.extend_from_slice(b"\\\""),
            '\\' => buf.extend_from_slice(b"\\\\"),
            '\x08' => buf.extend_from_slice(b"\\b"),
            '\x0C' => buf.extend_from_slice(b"\\f"),
            '\n' => buf.extend_from_slice(b"\\n"),
            '\r' => buf.extend_from_slice(b"\\r"),
            '\t' => buf.extend_from_slice(b"\\t"),
            c if (c as u32) < 0x20 => {
                buf.extend_from_slice(format!("\\u{:04x}", c as u32).as_bytes());
            }
            c => {
                let mut utf8_buf = [0u8; 4];
                buf.extend_from_slice(c.encode_utf8(&mut utf8_buf).as_bytes());
            }
        }
    }
    buf.push(b'"');
}

fn format_float(f: f64) -> String {
    if f.is_finite() {
        // Use enough precision to round-trip
        format!("{}", f)
    } else {
        "null".to_string() // JSON has no NaN or Infinity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sorted_keys() {
        let val = json!({"z": 1, "a": 2, "m": 3});
        let canonical = canonicalize(&val).unwrap();
        assert_eq!(
            String::from_utf8(canonical).unwrap(),
            r#"{"a":2,"m":3,"z":1}"#
        );
    }

    #[test]
    fn nested_sorted_keys() {
        let val = json!({"b": {"z": 1, "a": 2}, "a": 1});
        let canonical = canonicalize(&val).unwrap();
        assert_eq!(
            String::from_utf8(canonical).unwrap(),
            r#"{"a":1,"b":{"a":2,"z":1}}"#
        );
    }

    #[test]
    fn string_escaping() {
        let val = json!({"msg": "hello\nworld"});
        let canonical = canonicalize(&val).unwrap();
        assert_eq!(
            String::from_utf8(canonical).unwrap(),
            r#"{"msg":"hello\nworld"}"#
        );
    }

    #[test]
    fn array_order_preserved() {
        let val = json!([3, 1, 2]);
        let canonical = canonicalize(&val).unwrap();
        assert_eq!(String::from_utf8(canonical).unwrap(), "[3,1,2]");
    }

    #[test]
    fn deterministic() {
        let val = json!({"from": "alice", "to": "bob", "type": "HELLO", "ts": 1234567890});
        let a = canonicalize(&val).unwrap();
        let b = canonicalize(&val).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn null_bool() {
        let val = json!({"a": null, "b": true, "c": false});
        let canonical = canonicalize(&val).unwrap();
        assert_eq!(
            String::from_utf8(canonical).unwrap(),
            r#"{"a":null,"b":true,"c":false}"#
        );
    }
}
