use crate::models::DataTypeInfo;

/// Returns the list of data types supported by PostgreSQL.
/// Includes spatial types from PostGIS extension.
pub fn get_data_types() -> Vec<DataTypeInfo> {
    vec![
        // Numeric types
        DataTypeInfo {
            name: "SMALLINT".to_string(),
            category: "numeric".to_string(),
            requires_length: false,
            requires_precision: false,
            default_length: None,
        },
        DataTypeInfo {
            name: "INTEGER".to_string(),
            category: "numeric".to_string(),
            requires_length: false,
            requires_precision: false,
            default_length: None,
        },
        DataTypeInfo {
            name: "BIGINT".to_string(),
            category: "numeric".to_string(),
            requires_length: false,
            requires_precision: false,
            default_length: None,
        },
        DataTypeInfo {
            name: "DECIMAL".to_string(),
            category: "numeric".to_string(),
            requires_length: false,
            requires_precision: true,
            default_length: Some("10,2".to_string()),
        },
        DataTypeInfo {
            name: "NUMERIC".to_string(),
            category: "numeric".to_string(),
            requires_length: false,
            requires_precision: true,
            default_length: Some("10,2".to_string()),
        },
        DataTypeInfo {
            name: "REAL".to_string(),
            category: "numeric".to_string(),
            requires_length: false,
            requires_precision: false,
            default_length: None,
        },
        DataTypeInfo {
            name: "DOUBLE PRECISION".to_string(),
            category: "numeric".to_string(),
            requires_length: false,
            requires_precision: false,
            default_length: None,
        },
        DataTypeInfo {
            name: "SERIAL".to_string(),
            category: "numeric".to_string(),
            requires_length: false,
            requires_precision: false,
            default_length: None,
        },
        DataTypeInfo {
            name: "BIGSERIAL".to_string(),
            category: "numeric".to_string(),
            requires_length: false,
            requires_precision: false,
            default_length: None,
        },
        // String types
        DataTypeInfo {
            name: "VARCHAR".to_string(),
            category: "string".to_string(),
            requires_length: true,
            requires_precision: false,
            default_length: Some("255".to_string()),
        },
        DataTypeInfo {
            name: "CHAR".to_string(),
            category: "string".to_string(),
            requires_length: true,
            requires_precision: false,
            default_length: Some("10".to_string()),
        },
        DataTypeInfo {
            name: "TEXT".to_string(),
            category: "string".to_string(),
            requires_length: false,
            requires_precision: false,
            default_length: None,
        },
        // Boolean
        DataTypeInfo {
            name: "BOOLEAN".to_string(),
            category: "other".to_string(),
            requires_length: false,
            requires_precision: false,
            default_length: None,
        },
        // Date/Time types
        DataTypeInfo {
            name: "DATE".to_string(),
            category: "date".to_string(),
            requires_length: false,
            requires_precision: false,
            default_length: None,
        },
        DataTypeInfo {
            name: "TIME".to_string(),
            category: "date".to_string(),
            requires_length: false,
            requires_precision: false,
            default_length: None,
        },
        DataTypeInfo {
            name: "TIMESTAMP".to_string(),
            category: "date".to_string(),
            requires_length: false,
            requires_precision: false,
            default_length: None,
        },
        DataTypeInfo {
            name: "TIMESTAMPTZ".to_string(),
            category: "date".to_string(),
            requires_length: false,
            requires_precision: false,
            default_length: None,
        },
        DataTypeInfo {
            name: "INTERVAL".to_string(),
            category: "date".to_string(),
            requires_length: false,
            requires_precision: false,
            default_length: None,
        },
        // JSON types
        DataTypeInfo {
            name: "JSON".to_string(),
            category: "json".to_string(),
            requires_length: false,
            requires_precision: false,
            default_length: None,
        },
        DataTypeInfo {
            name: "JSONB".to_string(),
            category: "json".to_string(),
            requires_length: false,
            requires_precision: false,
            default_length: None,
        },
        // UUID
        DataTypeInfo {
            name: "UUID".to_string(),
            category: "other".to_string(),
            requires_length: false,
            requires_precision: false,
            default_length: None,
        },
        // Binary types
        DataTypeInfo {
            name: "BYTEA".to_string(),
            category: "binary".to_string(),
            requires_length: false,
            requires_precision: false,
            default_length: None,
        },
        // Spatial types (PostGIS)
        DataTypeInfo {
            name: "GEOMETRY".to_string(),
            category: "spatial".to_string(),
            requires_length: false,
            requires_precision: false,
            default_length: None,
        },
        DataTypeInfo {
            name: "GEOGRAPHY".to_string(),
            category: "spatial".to_string(),
            requires_length: false,
            requires_precision: false,
            default_length: None,
        },
        DataTypeInfo {
            name: "POINT".to_string(),
            category: "spatial".to_string(),
            requires_length: false,
            requires_precision: false,
            default_length: None,
        },
        DataTypeInfo {
            name: "LINESTRING".to_string(),
            category: "spatial".to_string(),
            requires_length: false,
            requires_precision: false,
            default_length: None,
        },
        DataTypeInfo {
            name: "POLYGON".to_string(),
            category: "spatial".to_string(),
            requires_length: false,
            requires_precision: false,
            default_length: None,
        },
    ]
}
