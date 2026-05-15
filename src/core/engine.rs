use crate::analyzer::bound::*;
use crate::sql::ast::{Expression, Operator, Value};
use crate::storage::table::{Column, deserialize_row, serialize_row};
use crate::types::{Cursor, QueryResult};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct EngineError {
    pub message: String,
}

fn eval_expr(expr: &Expression, row: &HashMap<&str, Value>) -> Result<Value, EngineError> {
    match expr {
        Expression::Literal(v) => Ok(v.clone()),

        Expression::Identifier(name) => {
            row.get(name.as_str()).cloned().ok_or_else(|| EngineError {
                message: format!("column '{}' not found in row", name),
            })
        }

        Expression::BinaryOp { op, lhs, rhs } => {
            let l = eval_expr(lhs, row)?;
            let r = eval_expr(rhs, row)?;

            match op {
                // Equality
                Operator::Eq => Ok(Value::Bool(l == r)),
                Operator::Neq => Ok(Value::Bool(l != r)),

                // Comparison (numbers only)
                Operator::Lt => match (l, r) {
                    (Value::Number(a), Value::Number(b)) => Ok(Value::Bool(a < b)),
                    _ => Err(EngineError {
                        message: "< requires numbers".to_string(),
                    }),
                },
                Operator::Gt => match (l, r) {
                    (Value::Number(a), Value::Number(b)) => Ok(Value::Bool(a > b)),
                    _ => Err(EngineError {
                        message: "> requires numbers".to_string(),
                    }),
                },
                Operator::Leq => match (l, r) {
                    (Value::Number(a), Value::Number(b)) => Ok(Value::Bool(a <= b)),
                    _ => Err(EngineError {
                        message: "<= requires numbers".to_string(),
                    }),
                },
                Operator::Geq => match (l, r) {
                    (Value::Number(a), Value::Number(b)) => Ok(Value::Bool(a >= b)),
                    _ => Err(EngineError {
                        message: ">= requires numbers".to_string(),
                    }),
                },

                // Arithmetic
                Operator::Add => match (l, r) {
                    (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a + b)),
                    _ => Err(EngineError {
                        message: "+ requires numbers".to_string(),
                    }),
                },
                Operator::Sub => match (l, r) {
                    (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a - b)),
                    _ => Err(EngineError {
                        message: "- requires numbers".to_string(),
                    }),
                },
                Operator::Mul => match (l, r) {
                    (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a * b)),
                    _ => Err(EngineError {
                        message: "* requires numbers".to_string(),
                    }),
                },
                Operator::Div => match (l, r) {
                    (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a / b)),
                    _ => Err(EngineError {
                        message: "/ requires numbers".to_string(),
                    }),
                },

                // Logical
                Operator::And => match (l, r) {
                    (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a && b)),
                    _ => Err(EngineError {
                        message: "&& requires booleans".to_string(),
                    }),
                },
                Operator::Or => match (l, r) {
                    (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a || b)),
                    _ => Err(EngineError {
                        message: "|| requires booleans".to_string(),
                    }),
                },
            }
        }
    }
}

pub fn execute_insert(stmt: &BoundInsertStatement) -> Result<u64, EngineError> {
    let table = stmt.table;
    let rowid = table.rows.get();

    // Prepend system columns (_rowid) to user values
    let mut all_values = vec![Value::Number(rowid as f64)];
    all_values.extend(stmt.values.iter().cloned());

    table
        .end()
        .with_row(|slot| serialize_row(&all_values, table.columns.values().collect(), slot));
    table.rows.set(rowid + 1);
    Ok(1)
}

pub fn execute_select(stmt: &BoundSelectStatement) -> Result<Vec<Vec<Value>>, EngineError> {
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

    let mut cursor = table.start();
    while !cursor.eot {
        let values = cursor.with_row(|slot| deserialize_row(&cols, slot));
        cursor.advance();

        // Build context for eval
        let row: HashMap<&str, Value> = col_names.iter().copied().zip(values.clone()).collect();

        // skip non-matching rows (expr is a WHERE clause here)
        if let Some(expr) = &stmt.expr {
            match eval_expr(expr, &row)? {
                Value::Bool(true) => {}
                Value::Bool(false) => continue,
                _ => {
                    return Err(EngineError {
                        message: "WHERE must be bool".into(),
                    });
                }
            }
        }

        // Project only requested columns by index
        let projected: Vec<Value> = projection_indices
            .iter()
            .map(|&i| values[i].clone())
            .collect();
        results.push(projected);
    }

    Ok(results)
}

pub fn execute(stmt: &BoundStatement) -> Result<QueryResult, EngineError> {
    match stmt {
        BoundStatement::Insert(s) => {
            let rows = execute_insert(s)?;
            Ok(QueryResult::Insert {
                rows_affected: rows,
            })
        }
        BoundStatement::Select(s) => {
            let rows = execute_select(s)?;
            Ok(QueryResult::Select { rows })
        }
    }
}
