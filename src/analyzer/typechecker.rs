use crate::analyzer::bound::*;
use crate::column::ColumnType;
use crate::error::Error;
use crate::sql::ast::{Expression, Operator};
use crate::storage::Table;
use crate::value::{Type, Value};

fn is_compatible_value(value: &Value, target: ColumnType) -> bool {
    match (value, target) {
        (Value::Number(_), ColumnType::Number) => true,
        (Value::Varchar(s), ColumnType::Varchar(n)) => s.len() <= n,
        (Value::Bool(_), ColumnType::Bool) => true,
        _ => false,
    }
}

fn typecheck_insert(stmt: &BoundInsertStatement) -> Result<(), Error> {
    let BoundInsertStatement { values, table } = stmt;
    let user_columns: Vec<_> = table.user_columns().collect();

    if values.len() != user_columns.len() {
        return Err(Error::WrongColumnCount {
            expected: user_columns.len(),
            got: values.len(),
        });
    }

    for (v, c) in std::iter::zip(values, user_columns) {
        if !is_compatible_value(v, c.column_type) {
            return Err(Error::TypeMismatch {
                expected: c.column_type.to_type(),
                got: v.typ(),
            });
        }
    }

    Ok(())
}
fn typecheck_select(stmt: &BoundSelectStatement) -> Result<(), Error> {
    // Since this is already bound, we don't need to validate columns or table really.
    if let Some(expr) = &stmt.expr {
        typecheck_expression(expr, stmt.table)?;
    }
    Ok(())
}
fn typecheck_delete(stmt: &BoundDeleteStatement) -> Result<(), Error> {
    // Since this is already bound, we don't need to validate table really.
    if let Some(expr) = &stmt.expr {
        typecheck_expression(expr, stmt.table)?;
    }
    Ok(())
}

// Returns the type that an expression evaluates to
fn typecheck_expression(expr: &Expression, table: &Table) -> Result<Type, Error> {
    match expr {
        Expression::Literal(value) => Ok(value.typ()),
        Expression::Identifier(name) => table
            .get_column(name)
            .map(|col| col.column_type.to_type())
            .ok_or_else(|| Error::ColumnNotFound(name.clone())),
        Expression::BinaryOp { op, lhs, rhs } => {
            let lhs_type = typecheck_expression(lhs, table)?;
            let rhs_type = typecheck_expression(rhs, table)?;

            match op {
                // Equality: any types, but must match
                Operator::Eq | Operator::Neq => {
                    if lhs_type != rhs_type {
                        return Err(Error::TypeMismatch {
                            expected: lhs_type,
                            got: rhs_type,
                        });
                    }
                    Ok(Type::Bool)
                }

                // Ordering: numbers only
                Operator::Lt | Operator::Gt | Operator::Leq | Operator::Geq => {
                    match (&lhs_type, &rhs_type) {
                        (Type::Number, Type::Number) => Ok(Type::Bool),
                        _ => Err(Error::TypeMismatch {
                            expected: Type::Number,
                            got: lhs_type,
                        }),
                    }
                }

                // Arithmetic: numbers only, returns number
                Operator::Add | Operator::Sub | Operator::Mul | Operator::Div => {
                    match (&lhs_type, &rhs_type) {
                        (Type::Number, Type::Number) => Ok(Type::Number),
                        _ => Err(Error::TypeMismatch {
                            expected: Type::Number,
                            got: lhs_type,
                        }),
                    }
                }

                // Logical: bools only
                Operator::And | Operator::Or => match (&lhs_type, &rhs_type) {
                    (Type::Bool, Type::Bool) => Ok(Type::Bool),
                    _ => Err(Error::TypeMismatch {
                        expected: Type::Bool,
                        got: lhs_type,
                    }),
                },
            }
        }
    }
}

pub fn typecheck(stmt: &BoundStatement) -> Result<(), Error> {
    match stmt {
        BoundStatement::Insert(s) => typecheck_insert(s),
        BoundStatement::Select(s) => typecheck_select(s),
        BoundStatement::Delete(s) => typecheck_delete(s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{ColumnType, Table};
    use ordered_float::OrderedFloat;

    fn make_test_table() -> Table {
        Table::new(
            "users".to_string(),
            [
                ("id".to_string(), ColumnType::Number),
                ("name".to_string(), ColumnType::Varchar(32)),
            ],
        )
    }

    #[test]
    fn test_typecheck_insert_success() {
        let table = make_test_table();
        let stmt = BoundInsertStatement {
            table: &table,
            values: vec![
                Value::Number(OrderedFloat(1.0)),
                Value::Varchar("alice".to_string()),
            ],
        };

        let result = typecheck_insert(&stmt);
        assert!(result.is_ok());
    }

    #[test]
    fn test_typecheck_insert_wrong_column_count() {
        let table = make_test_table();
        let stmt = BoundInsertStatement {
            table: &table,
            values: vec![Value::Number(OrderedFloat(1.0))], // missing second value
        };

        let result = typecheck_insert(&stmt);
        assert!(result.is_err());
    }

    #[test]
    fn test_typecheck_insert_type_mismatch() {
        let table = make_test_table();
        let stmt = BoundInsertStatement {
            table: &table,
            values: vec![
                Value::Varchar("not a number".to_string()), // should be Number
                Value::Varchar("alice".to_string()),
            ],
        };

        let result = typecheck_insert(&stmt);
        assert!(result.is_err());
    }

    #[test]
    fn test_typecheck_insert_varchar_too_long() {
        let table = make_test_table();
        let stmt = BoundInsertStatement {
            table: &table,
            values: vec![
                Value::Number(OrderedFloat(1.0)),
                Value::Varchar("a".repeat(100)), // exceeds Varchar(32)
            ],
        };

        let result = typecheck_insert(&stmt);
        assert!(result.is_err());
    }

    // Expression typechecking tests

    #[test]
    fn test_typecheck_expr_literal_number() {
        let table = make_test_table();
        let expr = Expression::Literal(Value::Number(OrderedFloat(42.0)));
        let result = typecheck_expression(&expr, &table).unwrap();
        assert_eq!(result, Type::Number);
    }

    #[test]
    fn test_typecheck_expr_literal_string() {
        let table = make_test_table();
        let expr = Expression::Literal(Value::Varchar("hello".to_string()));
        let result = typecheck_expression(&expr, &table).unwrap();
        assert!(matches!(result, Type::Varchar(_)));
    }

    #[test]
    fn test_typecheck_expr_identifier() {
        let table = make_test_table();
        let expr = Expression::Identifier("id".to_string());
        let result = typecheck_expression(&expr, &table).unwrap();
        assert_eq!(result, Type::Number);
    }

    #[test]
    fn test_typecheck_expr_unknown_column() {
        let table = make_test_table();
        let expr = Expression::Identifier("nonexistent".to_string());
        let result = typecheck_expression(&expr, &table);
        assert!(result.is_err());
    }

    #[test]
    fn test_typecheck_expr_eq_same_types() {
        let table = make_test_table();
        let expr = Expression::BinaryOp {
            op: Operator::Eq,
            lhs: Box::new(Expression::Literal(Value::Number(OrderedFloat(1.0)))),
            rhs: Box::new(Expression::Literal(Value::Number(OrderedFloat(2.0)))),
        };
        let result = typecheck_expression(&expr, &table).unwrap();
        assert_eq!(result, Type::Bool);
    }

    #[test]
    fn test_typecheck_expr_eq_different_types() {
        let table = make_test_table();
        let expr = Expression::BinaryOp {
            op: Operator::Eq,
            lhs: Box::new(Expression::Literal(Value::Number(OrderedFloat(1.0)))),
            rhs: Box::new(Expression::Literal(Value::Varchar("hello".to_string()))),
        };
        let result = typecheck_expression(&expr, &table);
        assert!(result.is_err());
    }

    #[test]
    fn test_typecheck_expr_arithmetic_numbers() {
        let table = make_test_table();
        let expr = Expression::BinaryOp {
            op: Operator::Add,
            lhs: Box::new(Expression::Literal(Value::Number(OrderedFloat(1.0)))),
            rhs: Box::new(Expression::Literal(Value::Number(OrderedFloat(2.0)))),
        };
        let result = typecheck_expression(&expr, &table).unwrap();
        assert_eq!(result, Type::Number);
    }

    #[test]
    fn test_typecheck_expr_arithmetic_string_fails() {
        let table = make_test_table();
        let expr = Expression::BinaryOp {
            op: Operator::Add,
            lhs: Box::new(Expression::Literal(Value::Varchar("a".to_string()))),
            rhs: Box::new(Expression::Literal(Value::Varchar("b".to_string()))),
        };
        let result = typecheck_expression(&expr, &table);
        assert!(result.is_err());
    }

    #[test]
    fn test_typecheck_expr_ordering_numbers() {
        let table = make_test_table();
        let expr = Expression::BinaryOp {
            op: Operator::Lt,
            lhs: Box::new(Expression::Literal(Value::Number(OrderedFloat(1.0)))),
            rhs: Box::new(Expression::Literal(Value::Number(OrderedFloat(2.0)))),
        };
        let result = typecheck_expression(&expr, &table).unwrap();
        assert_eq!(result, Type::Bool);
    }

    #[test]
    fn test_typecheck_expr_ordering_string_fails() {
        let table = make_test_table();
        let expr = Expression::BinaryOp {
            op: Operator::Lt,
            lhs: Box::new(Expression::Literal(Value::Varchar("a".to_string()))),
            rhs: Box::new(Expression::Literal(Value::Varchar("b".to_string()))),
        };
        let result = typecheck_expression(&expr, &table);
        assert!(result.is_err());
    }

    #[test]
    fn test_typecheck_expr_logical_bools() {
        let table = make_test_table();
        let expr = Expression::BinaryOp {
            op: Operator::And,
            lhs: Box::new(Expression::Literal(Value::Bool(true))),
            rhs: Box::new(Expression::Literal(Value::Bool(false))),
        };
        let result = typecheck_expression(&expr, &table).unwrap();
        assert_eq!(result, Type::Bool);
    }

    #[test]
    fn test_typecheck_expr_logical_number_fails() {
        let table = make_test_table();
        let expr = Expression::BinaryOp {
            op: Operator::And,
            lhs: Box::new(Expression::Literal(Value::Number(OrderedFloat(1.0)))),
            rhs: Box::new(Expression::Literal(Value::Number(OrderedFloat(2.0)))),
        };
        let result = typecheck_expression(&expr, &table);
        assert!(result.is_err());
    }

    #[test]
    fn test_typecheck_expr_column_comparison() {
        let table = make_test_table();
        // id == 42 (both numbers)
        let expr = Expression::BinaryOp {
            op: Operator::Eq,
            lhs: Box::new(Expression::Identifier("id".to_string())),
            rhs: Box::new(Expression::Literal(Value::Number(OrderedFloat(42.0)))),
        };
        let result = typecheck_expression(&expr, &table).unwrap();
        assert_eq!(result, Type::Bool);
    }
}
