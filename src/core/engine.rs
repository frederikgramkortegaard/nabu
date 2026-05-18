use super::evaluator::eval_expr;
use crate::analyzer::bound::*;
use crate::constants::NULL_BITMAP_SIZE;
use crate::error::Error;
use crate::storage::node::Node;
use crate::types::{Column, QueryResult, Row, Type, Value};
use ordered_float::OrderedFloat;
use std::collections::HashMap;

pub fn execute_insert(stmt: &BoundInsertStatement) -> Result<u64, Error> {
    let table = stmt.table;
    let rowid = table.next_row_id.get();
    table.next_row_id.set(rowid + 1);

    // Prepend system columns to user values
    let mut all_values = vec![
        Value::Number(OrderedFloat(rowid as f64)),    // _rowid
        Value::Number(OrderedFloat(0.0)),             // _tidmin (dummy)
        Value::Number(OrderedFloat(0.0)),             // _tidmax (dummy)
        Value::Bytes(vec![0u8; NULL_BITMAP_SIZE]),    // _null_bitmap (all non-null)
    ];
    all_values.extend(stmt.values.iter().cloned());

    let row = Row(all_values);
    let key = &row[stmt.table.primary_key_index];
    table.insert(key, &row)?;
    table.pager().borrow_mut().sync()?;
    Ok(1)
}
pub fn execute_delete(stmt: &BoundDeleteStatement) -> Result<u64, Error> {
    let table = stmt.table;

    let mut results = 0;
    let mut cursor = table.start()?;
    let cols: Vec<&Column> = table.columns.values().collect();
    let col_names: Vec<&str> = cols.iter().map(|c| c.name.as_str()).collect();
    while !cursor.eot {
        let node = cursor.read_node()?;
        let Node::Leaf { cells, .. } = node else {
            unreachable!("cursor should always point to a leaf node")
        };

        let (_, row) = &cells[cursor.cell_num];
        let values: HashMap<&str, Value> =
            col_names.iter().copied().zip(row.iter().cloned()).collect();

        if let Some(expr) = &stmt.expr {
            match eval_expr(expr, &values)? {
                Value::Bool(true) => {}
                Value::Bool(false) => {
                    cursor.advance()?;
                    continue;
                }
                other => {
                    return Err(Error::TypeMismatch {
                        expected: Type::Bool,
                        got: other.typ(),
                    });
                }
            }
        }
        let key = &row[table.primary_key_index];
        table.delete(key)?;
        cursor.refresh()?;
        results += 1;
    }

    table.pager().borrow_mut().sync()?;
    Ok(results)
}

pub fn execute_select(stmt: &BoundSelectStatement) -> Result<Vec<Vec<Value>>, Error> {
    let table = stmt.table;
    let cols: Vec<&Column> = table.columns.values().collect();
    let col_names: Vec<&str> = cols.iter().map(|c| c.name.as_str()).collect();

    // Pre-compute indices for projection
    let projection_indices: Vec<usize> = stmt
        .columns
        .iter()
        .filter_map(|c| col_names.iter().position(|&n| n == c.name))
        .collect();

    let mut results = vec![];
    let mut cursor = table.start()?;
    while !cursor.eot {
        let node = cursor.read_node()?;
        let Node::Leaf { cells, .. } = node else {
            unreachable!("cursor should always point to a leaf node")
        };

        let (_, row) = &cells[cursor.cell_num];
        let values: HashMap<&str, Value> =
            col_names.iter().copied().zip(row.iter().cloned()).collect();

        if let Some(expr) = &stmt.expr {
            match eval_expr(expr, &values)? {
                Value::Bool(true) => {}
                Value::Bool(false) => {
                    cursor.advance()?;
                    continue;
                }
                other => {
                    return Err(Error::TypeMismatch {
                        expected: Type::Bool,
                        got: other.typ(),
                    });
                }
            }
        }
        cursor.advance()?;

        // Project only requested columns by index
        let projected: Vec<Value> = projection_indices.iter().map(|&i| row[i].clone()).collect();
        results.push(projected);
    }

    Ok(results)
}

pub fn execute(stmt: &BoundStatement) -> Result<QueryResult, Error> {
    match stmt {
        BoundStatement::Insert(s) => {
            let rows = execute_insert(s)?;
            Ok(QueryResult::Insert {
                rows_affected: rows,
            })
        }
        BoundStatement::Delete(s) => {
            let rows = execute_delete(s)?;
            Ok(QueryResult::Delete {
                rows_affected: rows,
            })
        }
        BoundStatement::Select(s) => {
            let rows = execute_select(s)?;
            Ok(QueryResult::Select { rows })
        }
    }
}
