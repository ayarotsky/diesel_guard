use crate::error::{DieselGuardError, Result};
use sqlparser::ast::Statement;
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

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
            .map_err(|e| DieselGuardError::ParseError(e.to_string()))
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
}
