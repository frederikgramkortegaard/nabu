use crate::analyzer::bound::*;
use crate::column::Column;
use crate::error::Error;
use crate::node::Node;
use crate::sql::ast::{Expression, Operator};
use crate::types::QueryResult;
use crate::value::{Type, Value};
use ordered_float::OrderedFloat;
use std::collections::HashMap;

fn eval_expr(expr: &Expression, row: &HashMap<&str, Value>) -> Result<Value, Error> {
    match expr {
        Expression::Literal(v) => Ok(v.clone()),

        Expression::Identifier(name) => row
            .get(name.as_str())
            .cloned()
            .ok_or_else(|| Error::ColumnNotInRow(name.clone())),

        Expression::BinaryOp { op, lhs, rhs } => {
            let l = eval_expr(lhs, row)?;
            let r = eval_expr(rhs, row)?;

            match op {
                // Equality
                Operator::Eq => Ok(Value::Bool(l == r)),
                Operator::Neq => Ok(Value::Bool(l != r)),

                // Comparison (numbers only)
                Operator::Lt => match (&l, &r) {
                    (Value::Number(a), Value::Number(b)) => Ok(Value::Bool(a < b)),
                    _ => Err(Error::TypeMismatch {
                        expected: Type::Number,
                        got: l.typ(),
                    }),
                },
                Operator::Gt => match (&l, &r) {
                    (Value::Number(a), Value::Number(b)) => Ok(Value::Bool(a > b)),
                    _ => Err(Error::TypeMismatch {
                        expected: Type::Number,
                        got: l.typ(),
                    }),
                },
                Operator::Leq => match (&l, &r) {
                    (Value::Number(a), Value::Number(b)) => Ok(Value::Bool(a <= b)),
                    _ => Err(Error::TypeMismatch {
                        expected: Type::Number,
                        got: l.typ(),
                    }),
                },
                Operator::Geq => match (&l, &r) {
                    (Value::Number(a), Value::Number(b)) => Ok(Value::Bool(a >= b)),
                    _ => Err(Error::TypeMismatch {
                        expected: Type::Number,
                        got: l.typ(),
                    }),
                },

                // Arithmetic
                Operator::Add => match (&l, &r) {
                    (Value::Number(a), Value::Number(b)) => Ok(Value::Number(*a + *b)),
                    _ => Err(Error::TypeMismatch {
                        expected: Type::Number,
                        got: l.typ(),
                    }),
                },
                Operator::Sub => match (&l, &r) {
                    (Value::Number(a), Value::Number(b)) => Ok(Value::Number(*a - *b)),
                    _ => Err(Error::TypeMismatch {
                        expected: Type::Number,
                        got: l.typ(),
                    }),
                },
                Operator::Mul => match (&l, &r) {
                    (Value::Number(a), Value::Number(b)) => Ok(Value::Number(*a * *b)),
                    _ => Err(Error::TypeMismatch {
                        expected: Type::Number,
                        got: l.typ(),
                    }),
                },
                Operator::Div => match (&l, &r) {
                    (Value::Number(_), Value::Number(b)) if *b == 0.0 => Err(Error::DivisionByZero),
                    (Value::Number(a), Value::Number(b)) => Ok(Value::Number(*a / *b)),
                    _ => Err(Error::TypeMismatch {
                        expected: Type::Number,
                        got: l.typ(),
                    }),
                },

                // Logical
                Operator::And => match (&l, &r) {
                    (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(*a && *b)),
                    _ => Err(Error::TypeMismatch {
                        expected: Type::Bool,
                        got: l.typ(),
                    }),
                },
                Operator::Or => match (&l, &r) {
                    (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(*a || *b)),
                    _ => Err(Error::TypeMismatch {
                        expected: Type::Bool,
                        got: l.typ(),
                    }),
                },
            }
        }
    }
}

pub fn execute_insert(stmt: &BoundInsertStatement) -> Result<u64, Error> {
    let table = stmt.table;
    let rowid = table.rows.get();

    // Prepend system columns (_rowid) to user values
    let mut all_values = vec![Value::Number(OrderedFloat(rowid as f64))];
    all_values.extend(stmt.values.iter().cloned());

    let primary_key_index = stmt
        .table
        .columns
        .get_index_of(&stmt.table.primary_key_name)
        .ok_or_else(|| Error::ColumnNotFound(stmt.table.primary_key_name.clone()))?;

    let key = &all_values[primary_key_index];
    table.insert(key, &all_values)?;
    Ok(1)
}
pub fn execute_delete(stmt: &BoundDeleteStatement) -> Result<u64, Error> {
    let table = stmt.table;

    let mut results = 0;
    let mut cursor = table.start()?;
    let primary_key_index = stmt
        .table
        .columns
        .get_index_of(&stmt.table.primary_key_name)
        .ok_or_else(|| Error::ColumnNotFound(stmt.table.primary_key_name.clone()))?;
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
        let key = &row[primary_key_index];
        table.delete(key)?;
        cursor.refresh()?;
        results += 1;
    }

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
