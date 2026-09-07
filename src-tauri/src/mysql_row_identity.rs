/// Locate an encoded integer key without applying a function to the indexed
/// column. Images store textual bytes as X'..', not the integer's binary value.
/// Converting only the constant to DECIMAL avoids both hex-as-number semantics
/// and floating-point rounding above 2^53 (including BIGINT UNSIGNED).
///
/// Keep the byte-exact predicate as a residual guard: candidate lookup must not
/// weaken recovery conflict checks or accept a noncanonical recorded value.
/// Other types retain their existing comparison semantics.
pub(crate) fn encoded_key_condition(column: &str, data_type: &str, value: &str) -> String {
    let exact = format!("CAST({column} AS BINARY) <=> {value}");
    match data_type.to_ascii_lowercase().as_str() {
        "tinyint" | "smallint" | "mediumint" | "int" | "integer" | "bigint" | "year" => {
            format!(
                "({column} <=> CAST(CONVERT({value} USING ascii) AS DECIMAL(65,0)) AND {exact})"
            )
        }
        _ => exact,
    }
}

#[cfg(test)]
#[path = "mysql_row_identity_tests.rs"]
mod tests;
