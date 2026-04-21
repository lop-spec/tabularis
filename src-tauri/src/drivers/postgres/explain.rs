use super::client::{format_pg_error, query_all};
use super::helpers::escape_identifier;
use crate::explain_import::parse_postgres_json;
use crate::models::{ConnectionParams, ExplainPlan};
use crate::pool_manager::get_postgres_pool;

pub async fn explain_query(
    params: &ConnectionParams,
    query: &str,
    analyze: bool,
    schema: Option<&str>,
) -> Result<ExplainPlan, String> {
    let pool = get_postgres_pool(params).await?;

    if let Some(s) = schema {
        let search_path = format!("SET search_path TO \"{}\"", escape_identifier(s));
        query_all(&pool, &search_path, &[]).await?;
    }

    let explain_sql = if analyze {
        format!("EXPLAIN (FORMAT JSON, ANALYZE, BUFFERS) {}", query)
    } else {
        format!("EXPLAIN (FORMAT JSON) {}", query)
    };

    let rows = query_all(&pool, &explain_sql, &[]).await?;

    if rows.is_empty() {
        return Err("EXPLAIN returned no output".into());
    }

    // PostgreSQL returns a single row with a single text column containing JSON
    let plan_json_str: String = rows[0].try_get(0).map_err(|e| format_pg_error(&e))?;

    let mut plan = parse_postgres_json(&plan_json_str)?;
    plan.original_query = query.to_string();
    Ok(plan)
}
