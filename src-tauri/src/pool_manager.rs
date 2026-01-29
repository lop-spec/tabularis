use crate::models::ConnectionParams;
use once_cell::sync::Lazy;
use sqlx::{MySql, Pool, Postgres, Sqlite};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use urlencoding::encode;

type PoolMap<T> = Arc<RwLock<HashMap<String, Pool<T>>>>;

static MYSQL_POOLS: Lazy<PoolMap<MySql>> = Lazy::new(|| Arc::new(RwLock::new(HashMap::new())));
static POSTGRES_POOLS: Lazy<PoolMap<Postgres>> =
    Lazy::new(|| Arc::new(RwLock::new(HashMap::new())));
static SQLITE_POOLS: Lazy<PoolMap<Sqlite>> = Lazy::new(|| Arc::new(RwLock::new(HashMap::new())));

fn build_connection_key(params: &ConnectionParams) -> String {
    format!(
        "{}:{}:{}:{}",
        params.driver,
        params.host.as_deref().unwrap_or("localhost"),
        params.port.unwrap_or(0),
        params.database
    )
}

fn build_mysql_url(params: &ConnectionParams) -> String {
    let user = encode(params.username.as_deref().unwrap_or_default());
    let pass = encode(params.password.as_deref().unwrap_or_default());
    format!(
        "mysql://{}:{}@{}:{}/{}",
        user,
        pass,
        params.host.as_deref().unwrap_or("localhost"),
        params.port.unwrap_or(3306),
        params.database
    )
}

fn build_postgres_url(params: &ConnectionParams) -> String {
    let user = encode(params.username.as_deref().unwrap_or_default());
    let pass = encode(params.password.as_deref().unwrap_or_default());
    format!(
        "postgres://{}:{}@{}:{}/{}",
        user,
        pass,
        params.host.as_deref().unwrap_or("localhost"),
        params.port.unwrap_or(5432),
        params.database
    )
}

fn build_sqlite_url(params: &ConnectionParams) -> String {
    format!("sqlite://{}", params.database)
}

pub async fn get_mysql_pool(params: &ConnectionParams) -> Result<Pool<MySql>, String> {
    let key = build_connection_key(params);

    // Try to get existing pool
    {
        let pools = MYSQL_POOLS.read().await;
        if let Some(pool) = pools.get(&key) {
            return Ok(pool.clone());
        }
    }

    // Create new pool
    let url = build_mysql_url(params);
    let pool = sqlx::mysql::MySqlPoolOptions::new()
        .max_connections(10)
        .connect(&url)
        .await
        .map_err(|e| e.to_string())?;

    // Store pool
    {
        let mut pools = MYSQL_POOLS.write().await;
        pools.insert(key, pool.clone());
    }

    Ok(pool)
}

pub async fn get_postgres_pool(params: &ConnectionParams) -> Result<Pool<Postgres>, String> {
    let key = build_connection_key(params);

    // Try to get existing pool
    {
        let pools = POSTGRES_POOLS.read().await;
        if let Some(pool) = pools.get(&key) {
            return Ok(pool.clone());
        }
    }

    // Create new pool
    let url = build_postgres_url(params);
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(10)
        .connect(&url)
        .await
        .map_err(|e| e.to_string())?;

    // Store pool
    {
        let mut pools = POSTGRES_POOLS.write().await;
        pools.insert(key, pool.clone());
    }

    Ok(pool)
}

pub async fn get_sqlite_pool(params: &ConnectionParams) -> Result<Pool<Sqlite>, String> {
    let key = build_connection_key(params);

    // Try to get existing pool
    {
        let pools = SQLITE_POOLS.read().await;
        if let Some(pool) = pools.get(&key) {
            return Ok(pool.clone());
        }
    }

    // Create new pool
    let url = build_sqlite_url(params);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(5) // SQLite has lower concurrency needs
        .connect(&url)
        .await
        .map_err(|e| e.to_string())?;

    // Store pool
    {
        let mut pools = SQLITE_POOLS.write().await;
        pools.insert(key, pool.clone());
    }

    Ok(pool)
}

/// Close a specific connection pool
pub async fn close_pool(params: &ConnectionParams) {
    let key = build_connection_key(params);

    match params.driver.as_str() {
        "mysql" => {
            let mut pools = MYSQL_POOLS.write().await;
            if let Some(pool) = pools.remove(&key) {
                pool.close().await;
            }
        }
        "postgres" => {
            let mut pools = POSTGRES_POOLS.write().await;
            if let Some(pool) = pools.remove(&key) {
                pool.close().await;
            }
        }
        "sqlite" => {
            let mut pools = SQLITE_POOLS.write().await;
            if let Some(pool) = pools.remove(&key) {
                pool.close().await;
            }
        }
        _ => {}
    }
}

/// Close all connection pools (useful on app shutdown)
pub async fn close_all_pools() {
    {
        let mut pools = MYSQL_POOLS.write().await;
        for (_, pool) in pools.drain() {
            pool.close().await;
        }
    }
    {
        let mut pools = POSTGRES_POOLS.write().await;
        for (_, pool) in pools.drain() {
            pool.close().await;
        }
    }
    {
        let mut pools = SQLITE_POOLS.write().await;
        for (_, pool) in pools.drain() {
            pool.close().await;
        }
    }
}
