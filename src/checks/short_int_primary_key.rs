//! Detection for short integer types (SMALLINT, INT) used in primary keys.
//!
//! This check identifies primary key columns that use SMALLINT or INT/INTEGER data types,
//! which risk ID exhaustion. SMALLINT maxes out at ~32,767 records, and INT at ~2.1 billion.
//!
//! While 2.1 billion seems large, active applications can exhaust this faster than expected,
//! especially with high-frequency inserts, soft deletes, or partitioned data.
//!
//! Changing the type later requires an ALTER COLUMN TYPE operation that triggers a full
//! table rewrite with an ACCESS EXCLUSIVE lock, blocking all operations.

use crate::checks::Check;
use crate::violation::Violation;
use sqlparser::ast::{
    AlterTable, AlterTableOperation, ColumnDef, ColumnOption, DataType, Expr, ObjectName,
    Statement, TableConstraint,
};

pub struct ShortIntegerPrimaryKeyCheck;

impl Check for ShortIntegerPrimaryKeyCheck {
    fn check(&self, stmt: &Statement) -> Vec<Violation> {
        let mut violations = vec![];

        match stmt {
            Statement::CreateTable(create_table) => {
                // Check inline PRIMARY KEY constraints (id INT PRIMARY KEY)
                violations.extend(check_inline_pk_columns(
                    &create_table.name,
                    &create_table.columns,
                ));

                // Check separate PRIMARY KEY constraints (PRIMARY KEY (id))
                violations.extend(check_table_pk_constraints(
                    &create_table.name,
                    &create_table.columns,
                    &create_table.constraints,
                ));
            }
            Statement::AlterTable(AlterTable {
                name, operations, ..
            }) => {
                // Check inline PRIMARY KEY in ADD COLUMN
                violations.extend(check_alter_add_column_pk(name, operations));

                // Check ADD CONSTRAINT PRIMARY KEY
                violations.extend(check_alter_add_constraint_pk(name, operations));
            }
            _ => {}
        }

        violations
    }
}

/// Check if a data type is a short integer, returning (type_name, exhaustion_limit)
fn is_short_integer_type(data_type: &DataType) -> Option<(&'static str, &'static str)> {
    match data_type {
        DataType::SmallInt(_) => Some(("SMALLINT", "~32,767")),
        DataType::Int(_) => Some(("INT", "~2.1 billion")),
        DataType::Integer(_) => Some(("INTEGER", "~2.1 billion")),
        DataType::Int2(_) => Some(("INT2", "~32,767")),
        DataType::Int4(_) => Some(("INT4", "~2.1 billion")),
        _ => None,
    }
}

/// Extract column name from an index/constraint column expression
fn extract_column_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Identifier(ident) => Some(ident.to_string()),
        Expr::CompoundIdentifier(idents) => {
            // Handle qualified names like schema.table.column - take last part
            idents.last().map(|i| i.to_string())
        }
        _ => None, // Complex expressions in PK - rare, skip
    }
}

/// Check inline PRIMARY KEY constraints in column definitions
fn check_inline_pk_columns(table_name: &ObjectName, columns: &[ColumnDef]) -> Vec<Violation> {
    columns
        .iter()
        .filter_map(|col| {
            // Check if column has PRIMARY KEY constraint
            let is_primary_key = col
                .options
                .iter()
                .any(|opt| matches!(opt.option, ColumnOption::PrimaryKey(_)));

            if !is_primary_key {
                return None;
            }

            // Check if data type is short integer
            is_short_integer_type(&col.data_type).map(|(type_name, limit)| {
                create_violation(
                    table_name.to_string(),
                    col.name.to_string(),
                    type_name,
                    limit,
                )
            })
        })
        .collect()
}

/// Check separate PRIMARY KEY table constraints
fn check_table_pk_constraints(
    table_name: &ObjectName,
    columns: &[ColumnDef],
    constraints: &[TableConstraint],
) -> Vec<Violation> {
    let mut violations = vec![];

    for constraint in constraints {
        if let TableConstraint::PrimaryKey(pk_constraint) = constraint {
            // Extract column names from PRIMARY KEY constraint
            for pk_col in &pk_constraint.columns {
                if let Some(pk_col_name) = extract_column_name(&pk_col.column.expr) {
                    // Find the column definition
                    if let Some(col_def) =
                        columns.iter().find(|c| c.name.to_string() == pk_col_name)
                    {
                        if let Some((type_name, limit)) = is_short_integer_type(&col_def.data_type)
                        {
                            violations.push(create_violation(
                                table_name.to_string(),
                                pk_col_name,
                                type_name,
                                limit,
                            ));
                        }
                    }
                }
            }
        }
    }

    violations
}

/// Check ALTER TABLE ADD COLUMN with PRIMARY KEY
fn check_alter_add_column_pk(
    table_name: &ObjectName,
    operations: &[AlterTableOperation],
) -> Vec<Violation> {
    operations
        .iter()
        .filter_map(|op| {
            let AlterTableOperation::AddColumn { column_def, .. } = op else {
                return None;
            };

            // Check if column has PRIMARY KEY constraint
            let is_primary_key = column_def
                .options
                .iter()
                .any(|opt| matches!(opt.option, ColumnOption::PrimaryKey(_)));

            if !is_primary_key {
                return None;
            }

            // Check if data type is short integer
            is_short_integer_type(&column_def.data_type).map(|(type_name, limit)| {
                create_violation(
                    table_name.to_string(),
                    column_def.name.to_string(),
                    type_name,
                    limit,
                )
            })
        })
        .collect()
}

/// Check ALTER TABLE ADD CONSTRAINT PRIMARY KEY
///
/// This handles cases like:
/// - ALTER TABLE foo ADD CONSTRAINT pk_foo PRIMARY KEY (id);
/// - ALTER TABLE foo ADD COLUMN id INT, ADD CONSTRAINT pk_foo PRIMARY KEY (id);
fn check_alter_add_constraint_pk(
    table_name: &ObjectName,
    operations: &[AlterTableOperation],
) -> Vec<Violation> {
    // First, collect columns being added in this ALTER TABLE statement
    let added_columns: Vec<&ColumnDef> = operations
        .iter()
        .filter_map(|op| match op {
            AlterTableOperation::AddColumn { column_def, .. } => Some(column_def),
            _ => None,
        })
        .collect();

    // If no columns are being added, we can't determine types from this statement alone
    // (the columns would already exist in the table, and we don't track table state)
    if added_columns.is_empty() {
        return vec![];
    }

    // Now check for PRIMARY KEY constraints being added
    let mut violations = vec![];

    for operation in operations {
        if let AlterTableOperation::AddConstraint {
            constraint: TableConstraint::PrimaryKey(pk_constraint),
            ..
        } = operation
        {
            // Check each column in the PRIMARY KEY constraint
            for pk_col in &pk_constraint.columns {
                if let Some(pk_col_name) = extract_column_name(&pk_col.column.expr) {
                    // Find if this column was added in the same ALTER TABLE
                    if let Some(col_def) = added_columns
                        .iter()
                        .find(|c| c.name.to_string() == pk_col_name)
                    {
                        if let Some((type_name, limit)) = is_short_integer_type(&col_def.data_type)
                        {
                            violations.push(create_violation(
                                table_name.to_string(),
                                pk_col_name,
                                type_name,
                                limit,
                            ));
                        }
                    }
                }
            }
        }
    }

    violations
}

/// Create a violation for a short integer primary key
fn create_violation(
    table_name: String,
    column_name: String,
    type_name: &str,
    limit: &str,
) -> Violation {
    Violation::new(
        "Short integer primary key",
        format!(
            "Using {type_name} for primary key column '{column}' on table '{table}' risks ID exhaustion at {limit} records. \
            {type_name} can be quickly exhausted in production applications. \
            Changing the type later requires an ALTER COLUMN TYPE operation that triggers a full table rewrite with an \
            ACCESS EXCLUSIVE lock, blocking all operations. Duration depends on table size.",
            type_name = type_name,
            column = column_name,
            table = table_name,
            limit = limit
        ),
        format!(
            r#"Use BIGINT for primary keys to avoid ID exhaustion:

Instead of:
   CREATE TABLE {table} ({column} {type_name} PRIMARY KEY);

Use:
   CREATE TABLE {table} ({column} BIGINT PRIMARY KEY);

BIGINT provides 8 bytes (range: -9.2 quintillion to 9.2 quintillion), which is effectively unlimited
for auto-incrementing IDs. The minimal storage overhead (4 extra bytes per row) is negligible.

If using SERIAL/SMALLSERIAL, use BIGSERIAL instead:
   {column} BIGSERIAL PRIMARY KEY

Note: If this is an intentionally small table (e.g., lookup table with <100 entries),
use 'safety-assured' to bypass this check."#,
            table = table_name,
            column = column_name,
            type_name = type_name
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assert_allows, assert_detects_violation};

    // === CREATE TABLE with inline PRIMARY KEY ===

    #[test]
    fn test_detects_create_table_int_primary_key() {
        assert_detects_violation!(
            ShortIntegerPrimaryKeyCheck,
            "CREATE TABLE users (id INT PRIMARY KEY);",
            "Short integer primary key"
        );
    }

    #[test]
    fn test_detects_create_table_integer_primary_key() {
        assert_detects_violation!(
            ShortIntegerPrimaryKeyCheck,
            "CREATE TABLE users (id INTEGER PRIMARY KEY);",
            "Short integer primary key"
        );
    }

    #[test]
    fn test_detects_create_table_smallint_primary_key() {
        assert_detects_violation!(
            ShortIntegerPrimaryKeyCheck,
            "CREATE TABLE users (id SMALLINT PRIMARY KEY);",
            "Short integer primary key"
        );
    }

    #[test]
    fn test_detects_create_table_int2_primary_key() {
        assert_detects_violation!(
            ShortIntegerPrimaryKeyCheck,
            "CREATE TABLE users (id INT2 PRIMARY KEY);",
            "Short integer primary key"
        );
    }

    #[test]
    fn test_detects_create_table_int4_primary_key() {
        assert_detects_violation!(
            ShortIntegerPrimaryKeyCheck,
            "CREATE TABLE users (id INT4 PRIMARY KEY);",
            "Short integer primary key"
        );
    }

    // === CREATE TABLE with separate PRIMARY KEY constraint ===

    #[test]
    fn test_detects_create_table_separate_pk_constraint() {
        assert_detects_violation!(
            ShortIntegerPrimaryKeyCheck,
            "CREATE TABLE users (id INT, name TEXT, PRIMARY KEY (id));",
            "Short integer primary key"
        );
    }

    #[test]
    fn test_detects_composite_primary_key_with_int() {
        use crate::checks::test_utils::parse_sql;

        let check = ShortIntegerPrimaryKeyCheck;
        let stmt = parse_sql(
            "CREATE TABLE events (tenant_id BIGINT, id INT, PRIMARY KEY (tenant_id, id));",
        );

        let violations = check.check(&stmt);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].operation, "Short integer primary key");
        assert!(violations[0].problem.contains("id"));
        assert!(violations[0].problem.contains("INT"));
    }

    #[test]
    fn test_detects_multiple_short_int_columns_in_composite_pk() {
        use crate::checks::test_utils::parse_sql;

        let check = ShortIntegerPrimaryKeyCheck;
        let stmt = parse_sql(
            "CREATE TABLE data (tenant_id INT, user_id SMALLINT, PRIMARY KEY (tenant_id, user_id));",
        );

        let violations = check.check(&stmt);
        assert_eq!(violations.len(), 2); // Both columns flagged
        assert!(violations.iter().any(|v| v.problem.contains("tenant_id")));
        assert!(violations.iter().any(|v| v.problem.contains("user_id")));
    }

    // === ALTER TABLE ADD COLUMN ===

    #[test]
    fn test_detects_alter_add_column_int_primary_key() {
        assert_detects_violation!(
            ShortIntegerPrimaryKeyCheck,
            "ALTER TABLE users ADD COLUMN id INT PRIMARY KEY;",
            "Short integer primary key"
        );
    }

    #[test]
    fn test_detects_alter_add_column_smallint_primary_key() {
        assert_detects_violation!(
            ShortIntegerPrimaryKeyCheck,
            "ALTER TABLE users ADD COLUMN id SMALLINT PRIMARY KEY;",
            "Short integer primary key"
        );
    }

    // === Safe cases (should not trigger) ===

    #[test]
    fn test_allows_bigint_primary_key() {
        assert_allows!(
            ShortIntegerPrimaryKeyCheck,
            "CREATE TABLE users (id BIGINT PRIMARY KEY);"
        );
    }

    #[test]
    fn test_allows_int8_primary_key() {
        assert_allows!(
            ShortIntegerPrimaryKeyCheck,
            "CREATE TABLE users (id INT8 PRIMARY KEY);"
        );
    }

    #[test]
    fn test_allows_serial_primary_key() {
        assert_allows!(
            ShortIntegerPrimaryKeyCheck,
            "CREATE TABLE users (id SERIAL PRIMARY KEY);"
        );
    }

    #[test]
    fn test_allows_bigserial_primary_key() {
        assert_allows!(
            ShortIntegerPrimaryKeyCheck,
            "CREATE TABLE users (id BIGSERIAL PRIMARY KEY);"
        );
    }

    #[test]
    fn test_allows_uuid_primary_key() {
        assert_allows!(
            ShortIntegerPrimaryKeyCheck,
            "CREATE TABLE users (id UUID PRIMARY KEY);"
        );
    }

    #[test]
    fn test_allows_int_column_without_primary_key() {
        assert_allows!(
            ShortIntegerPrimaryKeyCheck,
            "CREATE TABLE users (id BIGINT PRIMARY KEY, age INT);"
        );
    }

    #[test]
    fn test_allows_int_unique_not_primary() {
        assert_allows!(
            ShortIntegerPrimaryKeyCheck,
            "CREATE TABLE users (id BIGINT PRIMARY KEY, code INT UNIQUE);"
        );
    }

    #[test]
    fn test_allows_composite_pk_all_bigint() {
        assert_allows!(
            ShortIntegerPrimaryKeyCheck,
            "CREATE TABLE events (tenant_id BIGINT, id BIGINT, PRIMARY KEY (tenant_id, id));"
        );
    }

    #[test]
    fn test_ignores_other_statements() {
        assert_allows!(
            ShortIntegerPrimaryKeyCheck,
            "ALTER TABLE users DROP COLUMN age;"
        );
    }

    #[test]
    fn test_ignores_alter_add_column_without_pk() {
        assert_allows!(
            ShortIntegerPrimaryKeyCheck,
            "ALTER TABLE users ADD COLUMN age INT;"
        );
    }

    // === ALTER TABLE ADD CONSTRAINT ===

    #[test]
    fn test_detects_alter_add_constraint_primary_key() {
        assert_detects_violation!(
            ShortIntegerPrimaryKeyCheck,
            "ALTER TABLE users ADD COLUMN id INT, ADD CONSTRAINT pk_users PRIMARY KEY (id);",
            "Short integer primary key"
        );
    }

    #[test]
    fn test_detects_alter_add_constraint_smallint_pk() {
        assert_detects_violation!(
            ShortIntegerPrimaryKeyCheck,
            "ALTER TABLE users ADD COLUMN id SMALLINT, ADD CONSTRAINT pk_users PRIMARY KEY (id);",
            "Short integer primary key"
        );
    }

    #[test]
    fn test_detects_alter_add_constraint_composite_pk_with_int() {
        use crate::checks::test_utils::parse_sql;

        let check = ShortIntegerPrimaryKeyCheck;
        let stmt = parse_sql(
            "ALTER TABLE events ADD COLUMN tenant_id BIGINT, ADD COLUMN id INT, ADD CONSTRAINT pk_events PRIMARY KEY (tenant_id, id);",
        );

        let violations = check.check(&stmt);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].operation, "Short integer primary key");
        assert!(violations[0].problem.contains("id"));
        assert!(violations[0].problem.contains("INT"));
    }

    #[test]
    fn test_allows_alter_add_constraint_bigint_pk() {
        assert_allows!(
            ShortIntegerPrimaryKeyCheck,
            "ALTER TABLE users ADD COLUMN id BIGINT, ADD CONSTRAINT pk_users PRIMARY KEY (id);"
        );
    }

    #[test]
    fn test_ignores_alter_add_constraint_on_existing_column() {
        // Can't detect type when column already exists (not added in same statement)
        assert_allows!(
            ShortIntegerPrimaryKeyCheck,
            "ALTER TABLE users ADD CONSTRAINT pk_users PRIMARY KEY (id);"
        );
    }

    // === Exhaustion limit messages ===

    #[test]
    fn test_smallint_shows_correct_limit() {
        use crate::checks::test_utils::parse_sql;

        let check = ShortIntegerPrimaryKeyCheck;
        let stmt = parse_sql("CREATE TABLE users (id SMALLINT PRIMARY KEY);");
        let violations = check.check(&stmt);

        assert_eq!(violations.len(), 1);
        assert!(violations[0].problem.contains("~32,767"));
    }

    #[test]
    fn test_int_shows_correct_limit() {
        use crate::checks::test_utils::parse_sql;

        let check = ShortIntegerPrimaryKeyCheck;
        let stmt = parse_sql("CREATE TABLE users (id INT PRIMARY KEY);");
        let violations = check.check(&stmt);

        assert_eq!(violations.len(), 1);
        assert!(violations[0].problem.contains("~2.1 billion"));
    }
}
