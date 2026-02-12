# Database Drivers Architecture

This directory contains database driver implementations for Tabularis. Each driver is organized as a separate module to support future plugin-based architecture.

## Current Structure

```
drivers/
├── common.rs           # Shared utilities for all drivers
├── postgres/
│   ├── mod.rs         # PostgreSQL driver implementation
│   └── types.rs       # PostgreSQL data types registry
├── mysql/
│   ├── mod.rs         # MySQL/MariaDB driver implementation
│   └── types.rs       # MySQL data types registry
└── sqlite/
    ├── mod.rs         # SQLite driver implementation
    └── types.rs       # SQLite data types registry
```

## Driver Responsibilities

Each driver module must implement:

1. **Database Operations** (in `mod.rs`):
   - `get_schemas()` - List available schemas
   - `get_databases()` - List available databases
   - `get_tables()` - List tables in a schema/database
   - `get_columns()` - Get column information for a table
   - `get_foreign_keys()` - Get foreign key relationships
   - `get_indexes()` - Get index information
   - `execute_query()` - Execute SQL queries
   - `get_views()` - List views
   - `get_view_definition()` - Get view SQL definition
   - `get_routines()` - List stored procedures/functions
   - `get_routine_parameters()` - Get routine parameters
   - `get_routine_definition()` - Get routine SQL definition

2. **Data Type Registry** (in `types.rs`):
   - `get_data_types()` - Returns `Vec<DataTypeInfo>` with supported types
   - Each type includes:
     - `name`: Type name (e.g., "VARCHAR", "INTEGER")
     - `category`: Type category ("numeric", "string", "date", "binary", "json", "spatial", "other")
     - `requires_length`: Whether the type requires a length parameter
     - `requires_precision`: Whether the type requires precision (e.g., DECIMAL(10,2))
     - `default_length`: Optional default length/precision string

## Adding a New Driver (Future Plugin System)

To add support for a new database:

1. Create a new directory: `drivers/your_driver/`

2. Implement `mod.rs` with all required functions:
```rust
use crate::models::{ConnectionParams, TableInfo, TableColumn, ...};

pub async fn get_tables(params: &ConnectionParams) -> Result<Vec<TableInfo>, String> {
    // Your implementation
}

// ... implement other required functions
```

3. Create `types.rs` with data type definitions:
```rust
use crate::models::DataTypeInfo;

pub fn get_data_types() -> Vec<DataTypeInfo> {
    vec![
        DataTypeInfo {
            name: "INT".to_string(),
            category: "numeric".to_string(),
            requires_length: false,
            requires_precision: false,
            default_length: None,
        },
        // ... more types
    ]
}
```

4. Register the driver in `commands.rs`:
```rust
match driver.to_lowercase().as_str() {
    "your_driver" => your_driver::types::get_data_types(),
    // ...
}
```

## Future Plugin Architecture

The current structure is designed to support a future plugin system where:

- Drivers can be compiled as separate dynamic libraries (`.so`, `.dylib`, `.dll`)
- Plugins can be loaded at runtime from a plugins directory
- Each driver implements a common trait/interface
- The type registry is part of the driver's public API
- Plugins can be installed/updated independently

### Planned Plugin Interface

```rust
pub trait DatabaseDriver {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn get_data_types(&self) -> Vec<DataTypeInfo>;
    async fn connect(&self, params: &ConnectionParams) -> Result<Connection, String>;
    async fn get_tables(&self, conn: &Connection) -> Result<Vec<TableInfo>, String>;
    // ... other methods
}
```

## Type Categories

- **numeric**: Integer and floating-point types (INT, BIGINT, DECIMAL, FLOAT)
- **string**: Text types (VARCHAR, TEXT, CHAR)
- **date**: Date and time types (DATE, TIMESTAMP, DATETIME)
- **binary**: Binary data types (BLOB, BYTEA, VARBINARY)
- **json**: JSON data types (JSON, JSONB)
- **spatial**: Geometric/spatial types (GEOMETRY, POINT, POLYGON)
- **other**: Types that don't fit other categories (BOOLEAN, UUID)

## Best Practices

1. **Error Handling**: Always return descriptive error messages
2. **Logging**: Use `log::debug!()`, `log::info!()`, `log::warn!()` appropriately
3. **Connection Pooling**: Use the pool manager from `crate::pool_manager`
4. **Type Safety**: Leverage Rust's type system for compile-time guarantees
5. **Async Operations**: Use async/await for all database operations
6. **Testing**: Include unit tests in each driver module

## Data Type Considerations

When defining data types for your driver:

- Include all commonly used types
- Set `requires_length: true` for types like VARCHAR that need a length
- Set `requires_precision: true` for types like DECIMAL that need precision
- Provide sensible `default_length` values (e.g., "255" for VARCHAR, "10,2" for DECIMAL)
- Group types by category for better UI organization
- Document any driver-specific behaviors or limitations
