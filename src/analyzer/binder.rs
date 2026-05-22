use crate::analyzer::bound::*;
use crate::error::Error;
use crate::sql::ast::*;
use crate::storage::{Database, Table};
use crate::types::Column;

/// Resolve a qualified identifier to a column reference.
/// - If 1 table: qualifier is optional, looks up column in that table
/// - If >1 tables: qualifier is required, finds table by name then column
pub fn resolve_column<'a>(
    id: &QualifiedIdentifier,
    tables: &[&'a Table],
) -> Result<&'a Column, Error> {
    let QualifiedIdentifier { qualifier, name } = id;

    if tables.is_empty() {
        return Err(Error::TableNotInScope("no tables in scope".into()));
    }

    if tables.len() == 1 {
        tables[0]
            .get_column(name.as_str())
            .ok_or_else(|| Error::ColumnNotFound(name.clone()))
    } else {
        let qualifier = qualifier
            .as_ref()
            .ok_or_else(|| Error::QualifierRequired { column: name.clone() })?;
        let table = tables
            .iter()
            .find(|t| t.name == *qualifier)
            .ok_or_else(|| Error::TableNotInScope(qualifier.clone()))?;

        table
            .get_column(name.as_str())
            .ok_or_else(|| Error::ColumnNotFoundInTable {
                column: name.clone(),
                table: qualifier.clone(),
            })
    }
}

/* CLAUSES */

fn bind_join_clause<'a>(
    clause: JoinClause,
    db: &'a Database,
) -> Result<BoundJoinClause<'a>, Error> {
    let on_table = db
        .get_table(clause.table.as_str())
        .ok_or_else(|| Error::TableNotFound(clause.table.clone()))?;

    Ok(BoundJoinClause {
        kind: clause.kind,
        on: clause.on,
        on_table,
    })
}

fn bind_where_clause(clause: WhereClause) -> Result<BoundWhereClause, Error> {
    Ok(BoundWhereClause(clause.0))
}

fn bind_limit_clause(limit: LimitClause) -> Result<BoundLimitClause, Error> {
    Ok(BoundLimitClause {
        limit: limit.limit,
        offset: limit.offset,
    })
}

fn bind_orderby_clause<'a>(
    orderby: OrderByClause,
    tables: &[&'a Table],
) -> Result<BoundOrderByClause<'a>, Error> {
    let column = resolve_column(&orderby.column, tables)?;
    Ok(BoundOrderByClause { column })
}

/* STATEMENTS */

fn bind_insert<'a>(stmt: InsertStatement, db: &'a Database) -> Result<BoundStatement<'a>, Error> {
    let table = db
        .get_table(stmt.table_name.as_str())
        .ok_or_else(|| Error::TableNotFound(stmt.table_name.clone()))?;

    Ok(BoundStatement::Insert(BoundInsertStatement {
        values: stmt.values,
        table,
    }))
}

fn bind_select<'a>(stmt: SelectStatement, db: &'a Database) -> Result<BoundStatement<'a>, Error> {
    let table = db
        .get_table(stmt.table.as_str())
        .ok_or_else(|| Error::TableNotFound(stmt.table.clone()))?;

    let mut columns: Vec<&Column> = vec![];
    for column_name in stmt.columns {
        if column_name == "*" {
            columns.extend(table.user_columns());
            break;
        }
        let col = table
            .get_user_column(&column_name)
            .ok_or_else(|| Error::ColumnNotFound(column_name.clone()))?;

        columns.push(col);
    }

    // Handle joins and get a reference to all related tables to the statement
    let join_clauses = stmt
        .joins
        .into_iter()
        .map(|clause| bind_join_clause(clause, db))
        .collect::<Result<Vec<_>, _>>()?;

    let mut tables: Vec<&Table> = join_clauses.iter().map(|jc| jc.on_table).collect();
    tables.extend(vec![table]);

    // Bind the remaining clauses
    let where_clause = stmt.where_clause.map(bind_where_clause).transpose()?;
    let limit_clause = stmt.limit_clause.map(bind_limit_clause).transpose()?;
    let orderby_clause = stmt
        .orderby_clause
        .map(|clause| bind_orderby_clause(clause, &tables))
        .transpose()?;

    Ok(BoundStatement::Select(BoundSelectStatement {
        table,
        columns,
        joins: vec![],
        where_clause,
        limit_clause,
        orderby_clause,
    }))
}

fn bind_delete<'a>(stmt: DeleteStatement, db: &'a Database) -> Result<BoundStatement<'a>, Error> {
    let table = db
        .get_table(stmt.table.as_str())
        .ok_or_else(|| Error::TableNotFound(stmt.table.clone()))?;

    let where_clause = stmt.where_clause.map(|wc| BoundWhereClause(wc.0));

    Ok(BoundStatement::Delete(BoundDeleteStatement {
        table,
        where_clause,
    }))
}

/* DISPATCH */

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
