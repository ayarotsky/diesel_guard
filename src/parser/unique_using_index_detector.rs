use regex::Regex;
use std::sync::LazyLock;

/// Regex pattern to detect ALTER TABLE ... ADD CONSTRAINT ... UNIQUE USING INDEX
static UNIQUE_USING_INDEX_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)ALTER\s+TABLE\s+\S+\s+ADD\s+CONSTRAINT\s+\S+\s+UNIQUE\s+USING\s+INDEX\s+\S+")
        .unwrap()
});

/// Check if SQL contains UNIQUE USING INDEX syntax that sqlparser can't parse
pub fn contains_unique_using_index(sql: &str) -> bool {
    UNIQUE_USING_INDEX_PATTERN.is_match(sql)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_unique_using_index() {
        assert!(contains_unique_using_index(
            "ALTER TABLE users ADD CONSTRAINT users_email_key UNIQUE USING INDEX users_email_idx;"
        ));
    }

    #[test]
    fn test_detects_case_insensitive() {
        assert!(contains_unique_using_index(
            "alter table users add constraint uk UNIQUE using index idx;"
        ));
    }

    #[test]
    fn test_ignores_regular_unique() {
        assert!(!contains_unique_using_index(
            "ALTER TABLE users ADD CONSTRAINT users_email_key UNIQUE (email);"
        ));
    }

    #[test]
    fn test_ignores_create_unique_index() {
        assert!(!contains_unique_using_index(
            "CREATE UNIQUE INDEX CONCURRENTLY idx ON users(email);"
        ));
    }
}
