use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use rust_decimal::Decimal;
use sqlx::Row;
use uuid::Uuid;

/// Extract value from MySQL row - supports all MySQL types including unsigned integers
pub fn extract_mysql_value(row: &sqlx::mysql::MySqlRow, index: usize) -> serde_json::Value {
    use sqlx::{Column, TypeInfo, ValueRef};

    // Get column info
    let col = row.columns().get(index);
    let col_name = col.map(|c| c.name()).unwrap_or("unknown");
    let col_type = col.map(|c| c.type_info().name()).unwrap_or("unknown");

    // Get the raw value to check if it's NULL
    let value_ref = row.try_get_raw(index).ok();
    if let Some(val_ref) = value_ref {
        if val_ref.is_null() {
            return serde_json::Value::Null;
        }
    }

    // DECIMAL/NUMERIC optimization
    if col_type == "DECIMAL" || col_type == "NEWDECIMAL" || col_type == "NUMERIC" {
        if let Ok(v) = row.try_get::<Decimal, _>(index) {
            return serde_json::Value::String(v.to_string());
        }
        // Fallback to string if Decimal fails
        if let Ok(v) = row.try_get::<String, _>(index) {
            return serde_json::Value::String(v);
        }
    }

    // For TIMESTAMP/DATETIME, try all possible representations
    if col_type == "TIMESTAMP" || col_type == "DATETIME" {
        // Try chrono types
        match row.try_get::<NaiveDateTime, _>(index) {
            Ok(v) => {
                return serde_json::Value::String(v.format("%Y-%m-%d %H:%M:%S").to_string());
            }
            Err(e) => eprintln!("[DEBUG] ✗ {} as NaiveDateTime: {}", col_name, e),
        }

        match row.try_get::<DateTime<Utc>, _>(index) {
            Ok(v) => {
                return serde_json::Value::String(v.format("%Y-%m-%d %H:%M:%S").to_string());
            }
            Err(e) => eprintln!("[DEBUG] ✗ {} as DateTime<Utc>: {}", col_name, e),
        }

        // Try as string
        match row.try_get::<String, _>(index) {
            Ok(v) => {
                // Try to parse typical SQL string formats to clean them up if they look like ISO
                if let Ok(dt) = NaiveDateTime::parse_from_str(&v, "%Y-%m-%dT%H:%M:%S%.f") {
                    return serde_json::Value::String(dt.format("%Y-%m-%d %H:%M:%S").to_string());
                }
                if let Ok(dt) = NaiveDateTime::parse_from_str(&v, "%Y-%m-%dT%H:%M:%S") {
                    return serde_json::Value::String(dt.format("%Y-%m-%d %H:%M:%S").to_string());
                }
                return serde_json::Value::String(v);
            }
            Err(e) => eprintln!("[DEBUG] ✗ {} as String: {}", col_name, e),
        }

        // Try as i64 (unix timestamp)
        match row.try_get::<i64, _>(index) {
            Ok(v) => {
                return serde_json::Value::from(v);
            }
            Err(e) => eprintln!("[DEBUG] ✗ {} as i64: {}", col_name, e),
        }
    }

    // For BLOB/BINARY types, try to extract as text first, then as binary
    if col_type.contains("BLOB") || col_type.contains("BINARY") {
        // First try as Vec<u8> (native binary format)
        match row.try_get::<Vec<u8>, _>(index) {
            Ok(v) => {
                // Try to decode as UTF-8 string first (many BLOBs contain text/JSON)
                if let Ok(s) = String::from_utf8(v.clone()) {
                    return serde_json::Value::String(s);
                }

                // If not valid UTF-8, encode as base64
                return serde_json::Value::String(base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    v,
                ));
            }
            Err(e) => eprintln!("[DEBUG] ✗ {} as Vec<u8>: {}", col_name, e),
        }

        // Try as string directly (for text-based binary data)
        match row.try_get::<String, _>(index) {
            Ok(v) => {
                return serde_json::Value::String(v);
            }
            Err(e) => eprintln!("[DEBUG] ✗ {} as String: {}", col_name, e),
        }
    }

    // For TEXT types (TINYTEXT, TEXT, MEDIUMTEXT, LONGTEXT), try string first
    if col_type.contains("TEXT") {
        match row.try_get::<String, _>(index) {
            Ok(v) => {
                return serde_json::Value::String(v);
            }
            Err(e) => eprintln!("[DEBUG] ✗ {} as String: {}", col_name, e),
        }

        // Fallback to Vec<u8> for non-UTF8 text
        match row.try_get::<Vec<u8>, _>(index) {
            Ok(v) => {
                // Try to convert to UTF-8 string first
                if let Ok(s) = String::from_utf8(v.clone()) {
                    return serde_json::Value::String(s);
                }
                // If not valid UTF-8, encode as base64
                return serde_json::Value::String(base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    v,
                ));
            }
            Err(e) => eprintln!("[DEBUG] ✗ {} as Vec<u8>: {}", col_name, e),
        }
    }

    // DateTime types (for other date columns)
    if let Ok(v) = row.try_get::<NaiveDateTime, _>(index) {
        return serde_json::Value::String(v.format("%Y-%m-%d %H:%M:%S").to_string());
    }
    if let Ok(v) = row.try_get::<NaiveDate, _>(index) {
        return serde_json::Value::String(v.to_string());
    }
    if let Ok(v) = row.try_get::<NaiveTime, _>(index) {
        return serde_json::Value::String(v.to_string());
    }

    // Unsigned integers (MySQL specific)
    if let Ok(v) = row.try_get::<u64, _>(index) {
        return serde_json::Value::from(v);
    }
    if let Ok(v) = row.try_get::<u32, _>(index) {
        return serde_json::Value::from(v);
    }
    if let Ok(v) = row.try_get::<u16, _>(index) {
        return serde_json::Value::from(v);
    }
    if let Ok(v) = row.try_get::<u8, _>(index) {
        return serde_json::Value::from(v);
    }

    // Signed integers
    if let Ok(v) = row.try_get::<i64, _>(index) {
        return serde_json::Value::from(v);
    }
    if let Ok(v) = row.try_get::<i32, _>(index) {
        return serde_json::Value::from(v);
    }
    if let Ok(v) = row.try_get::<i16, _>(index) {
        return serde_json::Value::from(v);
    }
    if let Ok(v) = row.try_get::<i8, _>(index) {
        return serde_json::Value::from(v);
    }

    // Decimal
    if let Ok(v) = row.try_get::<Decimal, _>(index) {
        return serde_json::Value::String(v.to_string());
    }

    // Floating point
    if let Ok(v) = row.try_get::<f64, _>(index) {
        return serde_json::Number::from_f64(v)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null);
    }
    if let Ok(v) = row.try_get::<f32, _>(index) {
        return serde_json::Number::from_f64(v as f64)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null);
    }

    // Boolean
    if let Ok(v) = row.try_get::<bool, _>(index) {
        return serde_json::Value::from(v);
    }

    // String
    if let Ok(v) = row.try_get::<String, _>(index) {
        return serde_json::Value::from(v);
    }

    // UUID
    if let Ok(v) = row.try_get::<Uuid, _>(index) {
        return serde_json::Value::String(v.to_string());
    }

    // Binary data
    if let Ok(v) = row.try_get::<Vec<u8>, _>(index) {
        return serde_json::Value::String(base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            v,
        ));
    }

    // Fallback
    let type_info = row.column(index).type_info();
    eprintln!(
        "[WARNING] Column '{}' [{}] type '{}' (TypeInfo: {:?}) could not be extracted. Raw value available: {:?}",
        col_name, index, col_type, type_info, row.try_get_raw(index).is_ok()
    );
    serde_json::Value::Null
}

/// Extract value from PostgreSQL row
pub fn extract_postgres_value(row: &sqlx::postgres::PgRow, index: usize) -> serde_json::Value {
    use sqlx::ValueRef;

    // Check for NULL first
    if let Ok(val_ref) = row.try_get_raw(index) {
        if val_ref.is_null() {
            return serde_json::Value::Null;
        }
    }

    // DateTime types FIRST
    if let Ok(v) = row.try_get::<DateTime<Utc>, _>(index) {
        return serde_json::Value::String(v.format("%Y-%m-%d %H:%M:%S").to_string());
    }
    if let Ok(v) = row.try_get::<NaiveDateTime, _>(index) {
        return serde_json::Value::String(v.format("%Y-%m-%d %H:%M:%S").to_string());
    }
    if let Ok(v) = row.try_get::<NaiveDate, _>(index) {
        return serde_json::Value::String(v.to_string());
    }
    if let Ok(v) = row.try_get::<NaiveTime, _>(index) {
        return serde_json::Value::String(v.to_string());
    }

    // Signed integers only
    if let Ok(v) = row.try_get::<i64, _>(index) {
        return serde_json::Value::from(v);
    }
    if let Ok(v) = row.try_get::<i32, _>(index) {
        return serde_json::Value::from(v);
    }
    if let Ok(v) = row.try_get::<i16, _>(index) {
        return serde_json::Value::from(v);
    }

    // Decimal
    if let Ok(v) = row.try_get::<Decimal, _>(index) {
        return serde_json::Value::String(v.to_string());
    }

    // Floating point
    if let Ok(v) = row.try_get::<f64, _>(index) {
        return serde_json::Number::from_f64(v)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null);
    }
    if let Ok(v) = row.try_get::<f32, _>(index) {
        return serde_json::Number::from_f64(v as f64)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null);
    }

    // Boolean
    if let Ok(v) = row.try_get::<bool, _>(index) {
        return serde_json::Value::from(v);
    }

    // String
    if let Ok(v) = row.try_get::<String, _>(index) {
        return serde_json::Value::from(v);
    }

    // UUID
    if let Ok(v) = row.try_get::<Uuid, _>(index) {
        return serde_json::Value::String(v.to_string());
    }

    // Binary data
    if let Ok(v) = row.try_get::<Vec<u8>, _>(index) {
        return serde_json::Value::String(base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            v,
        ));
    }

    // JSON
    if let Ok(v) = row.try_get::<serde_json::Value, _>(index) {
        return v;
    }

    serde_json::Value::Null
}

/// Extract value from SQLite row
pub fn extract_sqlite_value(row: &sqlx::sqlite::SqliteRow, index: usize) -> serde_json::Value {
    use sqlx::ValueRef;

    // Check for NULL first
    if let Ok(val_ref) = row.try_get_raw(index) {
        if val_ref.is_null() {
            return serde_json::Value::Null;
        }
    }

    // String first (SQLite stores dates as text)
    if let Ok(v) = row.try_get::<String, _>(index) {
        return serde_json::Value::from(v);
    }

    // Integers
    if let Ok(v) = row.try_get::<i64, _>(index) {
        return serde_json::Value::from(v);
    }
    if let Ok(v) = row.try_get::<i32, _>(index) {
        return serde_json::Value::from(v);
    }

    // Floating point
    if let Ok(v) = row.try_get::<f64, _>(index) {
        return serde_json::Number::from_f64(v)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null);
    }

    // Boolean
    if let Ok(v) = row.try_get::<bool, _>(index) {
        return serde_json::Value::from(v);
    }

    // Binary data
    if let Ok(v) = row.try_get::<Vec<u8>, _>(index) {
        return serde_json::Value::String(base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            v,
        ));
    }

    serde_json::Value::Null
}

pub fn is_select_query(query: &str) -> bool {
    query.trim_start().to_uppercase().starts_with("SELECT")
}

pub fn calculate_offset(page: u32, page_size: u32) -> u32 {
    (page - 1) * page_size
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_select_query() {
        assert!(is_select_query("SELECT * FROM users"));
        assert!(is_select_query("  select * from users"));
        assert!(is_select_query("\n\tSELECT id FROM posts"));
        assert!(!is_select_query("UPDATE users SET name = 'test'"));
        assert!(!is_select_query("DELETE FROM users"));
        assert!(!is_select_query("INSERT INTO users VALUES (1)"));
    }

    #[test]
    fn test_calculate_offset() {
        assert_eq!(calculate_offset(1, 100), 0);
        assert_eq!(calculate_offset(2, 100), 100);
        assert_eq!(calculate_offset(3, 50), 100);
        assert_eq!(calculate_offset(10, 25), 225);
    }
}
