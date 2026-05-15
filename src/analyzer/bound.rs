use crate::sql::ast::*;
use crate::storage::database::Database;
use crate::storage::table::{Column, Table};
#[derive(Debug)]
pub struct BoundInsertStatement<'a> {
    pub values: Vec<Value>,
    pub table: &'a Table,
}

#[derive(Debug)]
pub struct BoundSelectStatement<'a> {
    pub columns: Vec<&'a Column>,
    pub table: &'a Table,
    pub expr: Option<Box<Expression>>,
}

#[derive(Debug)]
pub enum BoundStatement<'a> {
    Insert(BoundInsertStatement<'a>),
    Select(BoundSelectStatement<'a>),
}

#[derive(Debug)]
pub struct BindingError {
    message: String,
}

fn bind_insert<'a>(
    stmt: InsertStatement,
    db: &'a Database,
) -> Result<BoundStatement<'a>, BindingError> {
    let table = db
        .get_table(stmt.table_name.as_str())
        .ok_or_else(|| BindingError {
            message: format!("table '{:?}' does not exist", stmt.table_name),
        })?;

    Ok(BoundStatement::Insert(BoundInsertStatement {
        values: stmt.values,
        table,
    }))
}

fn bind_select<'a>(
    stmt: SelectStatement,
    db: &'a Database,
) -> Result<BoundStatement<'a>, BindingError> {
    let table = db
        .get_table(stmt.table.as_str())
        .ok_or_else(|| BindingError {
            message: format!("table '{:?}' does not exist", stmt.table),
        })?;

    let mut columns: Vec<&Column> = vec![];
    for column_name in stmt.columns {
        let col = table.get_column(&column_name).ok_or_else(|| BindingError {
            message: format!(
                "coumn '{:?}' does not exist on table '{:?}'",
                column_name, table.name
            ),
        })?;

        columns.push(col);
    }

    Ok(BoundStatement::Select(BoundSelectStatement {
        table,
        columns,
        expr: stmt.expr,
    }))
}
pub fn bind<'a>(stmt: Statement, db: &'a Database) -> Result<BoundStatement<'a>, BindingError> {
    match stmt {
        Statement::Insert(s) => bind_insert(s, db),
        Statement::Select(s) => bind_select(s, db),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{ColumnType, Table};

    #[test]
    fn test_bind_insert_success() {
        let table = Table::new(
            "users".to_string(),
            [("id".to_string(), ColumnType::Number)],
        );
        let mut db = Database::new();
        db.add_table(&table).unwrap();

        let stmt = Statement::Insert(InsertStatement {
            table_name: "users".to_string(),
            values: vec![Value::Number(1.0)],
        });

        let result = bind(stmt, &db);
        assert!(result.is_ok());
    }

    #[test]
    fn test_bind_insert_table_not_found() {
        let db = Database::new();

        let stmt = Statement::Insert(InsertStatement {
            table_name: "nonexistent".to_string(),
            values: vec![],
        });

        let result = bind(stmt, &db);
        assert!(result.is_err());
    }
}
