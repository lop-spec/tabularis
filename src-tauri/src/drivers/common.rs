/// Check if a query is a SELECT statement
pub fn is_select_query(query: &str) -> bool {
    query.trim_start().to_uppercase().starts_with("SELECT")
}

/// Calculate offset for pagination
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
