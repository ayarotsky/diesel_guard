use crate::checks::CheckRegistry;
use crate::error::Result;
use crate::parser::SqlParser;
use crate::violation::Violation;
use std::fs;
use std::path::Path;

pub struct SafetyChecker {
    parser: SqlParser,
    registry: CheckRegistry,
}

impl SafetyChecker {
    pub fn new() -> Self {
        Self {
            parser: SqlParser::new(),
            registry: CheckRegistry::new(),
        }
    }

    /// Check SQL string for violations
    pub fn check_sql(&self, sql: &str) -> Result<Vec<Violation>> {
        let statements = self.parser.parse(sql)?;
        Ok(self.registry.check_statements(&statements))
    }

    /// Check a single migration file
    pub fn check_file(&self, path: &Path) -> Result<Vec<Violation>> {
        let sql = fs::read_to_string(path)?;
        self.check_sql(&sql)
    }

    /// Check all migration files in a directory
    pub fn check_directory(&self, dir: &Path) -> Result<Vec<(String, Vec<Violation>)>> {
        let mut results = vec![];

        if dir.is_dir() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_dir() {
                    // Check for up.sql in migration directories
                    let up_sql = path.join("up.sql");
                    if up_sql.exists() {
                        let violations = self.check_file(&up_sql)?;
                        if !violations.is_empty() {
                            results.push((up_sql.display().to_string(), violations));
                        }
                    }
                } else if path.extension().and_then(|s| s.to_str()) == Some("sql") {
                    let violations = self.check_file(&path)?;
                    if !violations.is_empty() {
                        results.push((path.display().to_string(), violations));
                    }
                }
            }
        }

        Ok(results)
    }

    /// Check a path (file or directory)
    pub fn check_path(&self, path: &Path) -> Result<Vec<(String, Vec<Violation>)>> {
        if path.is_dir() {
            self.check_directory(path)
        } else {
            let violations = self.check_file(path)?;
            if violations.is_empty() {
                Ok(vec![])
            } else {
                Ok(vec![(path.display().to_string(), violations)])
            }
        }
    }
}

impl Default for SafetyChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_safe_sql() {
        let checker = SafetyChecker::new();
        let sql = "ALTER TABLE users ADD COLUMN email VARCHAR(255);";
        let violations = checker.check_sql(sql).unwrap();
        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn test_check_unsafe_sql() {
        let checker = SafetyChecker::new();
        let sql = "ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;";
        let violations = checker.check_sql(sql).unwrap();
        assert_eq!(violations.len(), 1);
    }
}
