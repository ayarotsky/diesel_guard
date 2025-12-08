//! Detection for ADD COLUMN with SERIAL data types.
//!
//! This check identifies `ALTER TABLE` statements that add columns with SERIAL,
//! SMALLSERIAL, or BIGSERIAL data types, which trigger a full table rewrite.
//!
//! Adding a SERIAL column to an existing table requires PostgreSQL to:
//! 1. Create a new sequence
//! 2. Rewrite the entire table to populate the sequence values for existing rows
//! 3. Update all indexes
//!
//! This operation acquires an ACCESS EXCLUSIVE lock, blocking all operations.
//! Duration depends on table size and number of indexes.

use crate::checks::Check;
use crate::violation::Violation;
use sqlparser::ast::{AlterTableOperation, DataType, Statement};

pub struct AddSerialColumnCheck;

impl Check for AddSerialColumnCheck {
    fn check(&self, stmt: &Statement) -> Vec<Violation> {
        let Statement::AlterTable {
            name, operations, ..
        } = stmt
        else {
            return vec![];
        };

        let table_name = name.to_string();

        operations
            .iter()
            .filter_map(|op| {
                let AlterTableOperation::AddColumn { column_def, .. } = op else {
                    return None;
                };

                // Check if column uses SERIAL, SMALLSERIAL, or BIGSERIAL type
                let is_serial = matches!(
                    &column_def.data_type,
                    DataType::Custom(name, _) if is_serial_type(&name.to_string())
                );

                if !is_serial {
                    return None;
                }

                let column_name = &column_def.name;

                Some(Violation::new(
                    "ADD COLUMN with SERIAL",
                    format!(
                        "Adding column '{column}' with SERIAL type on table '{table}' requires a full table rewrite to populate sequence values for existing rows, \
                        which acquires an ACCESS EXCLUSIVE lock and blocks all operations. Duration depends on table size and number of indexes.",
                        column = column_name, table = table_name
                    ),
                    format!(r#"1. Create a sequence:
   CREATE SEQUENCE {table}_{column}_seq;

2. Add the column WITHOUT default (fast, no rewrite):
   ALTER TABLE {table} ADD COLUMN {column} INTEGER;

3. Backfill existing rows in batches (outside migration):
   UPDATE {table} SET {column} = nextval('{table}_{column}_seq') WHERE {column} IS NULL;

4. Set default for future inserts only:
   ALTER TABLE {table} ALTER COLUMN {column} SET DEFAULT nextval('{table}_{column}_seq');

5. Set NOT NULL if needed (PostgreSQL 11+: safe if all values present):
   ALTER TABLE {table} ALTER COLUMN {column} SET NOT NULL;

6. Set sequence ownership:
   ALTER SEQUENCE {table}_{column}_seq OWNED BY {table}.{column};"#,
                        table = table_name,
                        column = column_name
                    ),
                ))
            })
            .collect()
    }
}

/// Helper function to check if a type name is a SERIAL variant
fn is_serial_type(type_name: &str) -> bool {
    matches!(
        type_name.to_uppercase().as_str(),
        "SERIAL" | "SMALLSERIAL" | "BIGSERIAL"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assert_allows, assert_detects_violation};

    #[test]
    fn test_detects_add_column_with_serial() {
        assert_detects_violation!(
            AddSerialColumnCheck,
            "ALTER TABLE users ADD COLUMN id SERIAL;",
            "ADD COLUMN with SERIAL"
        );
    }

    #[test]
    fn test_detects_add_column_with_bigserial() {
        assert_detects_violation!(
            AddSerialColumnCheck,
            "ALTER TABLE users ADD COLUMN id BIGSERIAL;",
            "ADD COLUMN with SERIAL"
        );
    }

    #[test]
    fn test_detects_add_column_with_smallserial() {
        assert_detects_violation!(
            AddSerialColumnCheck,
            "ALTER TABLE users ADD COLUMN id SMALLSERIAL;",
            "ADD COLUMN with SERIAL"
        );
    }

    #[test]
    fn test_allows_add_column_with_integer() {
        assert_allows!(
            AddSerialColumnCheck,
            "ALTER TABLE users ADD COLUMN count INTEGER;"
        );
    }

    #[test]
    fn test_allows_create_table_with_serial() {
        assert_allows!(
            AddSerialColumnCheck,
            "CREATE TABLE users (id SERIAL PRIMARY KEY);"
        );
    }

    #[test]
    fn test_ignores_other_statements() {
        assert_allows!(
            AddSerialColumnCheck,
            "CREATE INDEX idx_users_email ON users(email);"
        );
    }
}
