use crate::error::{DieselGuardError, Result};
use sqlparser::ast::Statement;
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

pub mod comment_parser;

pub use comment_parser::IgnoreRange;

/// Parsed SQL with metadata for safety-assured handling
pub struct ParsedSql {
    pub statements: Vec<Statement>,
    pub sql: String,
    pub ignore_ranges: Vec<IgnoreRange>,
}

pub struct SqlParser {
    dialect: PostgreSqlDialect,
}

impl SqlParser {
    pub fn new() -> Self {
        Self {
            dialect: PostgreSqlDialect {},
        }
    }

    /// Parse SQL string into AST statements
    pub fn parse(&self, sql: &str) -> Result<Vec<Statement>> {
        Parser::parse_sql(&self.dialect, sql)
            .map_err(|e| DieselGuardError::parse_error(e.to_string()))
    }

    /// Parse SQL with metadata for safety-assured blocks
    pub fn parse_with_metadata(&self, sql: &str) -> Result<ParsedSql> {
        let statements = self.parse(sql)?;
        let ignore_ranges = comment_parser::CommentParser::parse_ignore_ranges(sql)?;

        Ok(ParsedSql {
            statements,
            sql: sql.to_string(),
            ignore_ranges,
        })
    }
}

impl Default for SqlParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_select() {
        let parser = SqlParser::new();
        let result = parser.parse("SELECT * FROM users;");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_alter_table() {
        let parser = SqlParser::new();
        let result = parser.parse("ALTER TABLE users ADD COLUMN email VARCHAR(255);");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_invalid_sql() {
        let parser = SqlParser::new();
        let result = parser.parse("INVALID SQL HERE");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_with_metadata() {
        let parser = SqlParser::new();
        let sql = r#"
-- safety-assured:start
ALTER TABLE users DROP COLUMN email;
-- safety-assured:end
        "#;

        let result = parser.parse_with_metadata(sql).unwrap();
        assert_eq!(result.statements.len(), 1);
        assert_eq!(result.ignore_ranges.len(), 1);
        assert!(!result.sql.is_empty());
    }

    #[test]
    fn test_parse_with_metadata_no_blocks() {
        let parser = SqlParser::new();
        let sql = "ALTER TABLE users DROP COLUMN email;";

        let result = parser.parse_with_metadata(sql).unwrap();
        assert_eq!(result.statements.len(), 1);
        assert_eq!(result.ignore_ranges.len(), 0);
        assert_eq!(result.sql, sql);
    }
}
