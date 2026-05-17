use crate::error::Error;
use crate::sql::ast::*;
use crate::storage::{Database, Table};
use crate::types::Column;
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
pub struct BoundDeleteStatement<'a> {
    pub table: &'a Table,
    pub expr: Option<Box<Expression>>,
}

#[derive(Debug)]
pub enum BoundStatement<'a> {
    Insert(BoundInsertStatement<'a>),
    Select(BoundSelectStatement<'a>),
    Delete(BoundDeleteStatement<'a>),
}

fn bind_insert<'a>(
    stmt: InsertStatement,
    db: &'a Database,
) -> Result<BoundStatement<'a>, Error> {
    let table = db
        .get_table(stmt.table_name.as_str())
        .ok_or_else(|| Error::TableNotFound(stmt.table_name.clone()))?;

    Ok(BoundStatement::Insert(BoundInsertStatement {
        values: stmt.values,
        table,
    }))
}

fn bind_select<'a>(
    stmt: SelectStatement,
    db: &'a Database,
) -> Result<BoundStatement<'a>, Error> {
    let table = db
        .get_table(stmt.table.as_str())
        .ok_or_else(|| Error::TableNotFound(stmt.table.clone()))?;

    let mut columns: Vec<&Column> = vec![];
    for column_name in stmt.columns {
        let col = table.get_column(&column_name).ok_or_else(|| Error::ColumnNotFound(column_name.clone()))?;

        columns.push(col);
    }

    Ok(BoundStatement::Select(BoundSelectStatement {
        table,
        columns,
        expr: stmt.expr,
    }))
}
fn bind_delete<'a>(
    stmt: DeleteStatement,
    db: &'a Database,
) -> Result<BoundStatement<'a>, Error> {
    let table = db
        .get_table(stmt.table.as_str())
        .ok_or_else(|| Error::TableNotFound(stmt.table.clone()))?;

    Ok(BoundStatement::Delete(BoundDeleteStatement {
        table,
        expr: stmt.expr,
    }))
}
pub fn bind<'a>(stmt: Statement, db: &'a Database) -> Result<BoundStatement<'a>, Error> {
    match stmt {
        Statement::Insert(s) => bind_insert(s, db),
        Statement::Select(s) => bind_select(s, db),
        Statement::Delete(s) => bind_delete(s, db),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::TableBuilder;
    use crate::types::ColumnType;
    use ordered_float::OrderedFloat;

    #[test]
    fn test_bind_insert_success() {
        let mut db = Database::memory().unwrap();
        db.create_table(TableBuilder::new("users").column("id", ColumnType::Number))
            .unwrap();

        let stmt = Statement::Insert(InsertStatement {
            table_name: "users".to_string(),
            values: vec![Value::Number(OrderedFloat(1.0))],
        });

        let result = bind(stmt, &db);
        assert!(result.is_ok());
    }

    #[test]
    fn test_bind_insert_table_not_found() {
        let db = Database::memory().unwrap();

        let stmt = Statement::Insert(InsertStatement {
            table_name: "nonexistent".to_string(),
            values: vec![],
        });

        let result = bind(stmt, &db);
        assert!(result.is_err());
    }
}
