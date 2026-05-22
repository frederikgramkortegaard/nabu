use crate::analyzer::binder::resolve_column;
use crate::analyzer::bound::*;
use crate::error::Error;
use crate::sql::ast::{Expression, Operator};
use crate::storage::{Database, Table};
use crate::types::{ColumnType, Type, Value};

fn check_value(value: &Value, target: ColumnType) -> Result<(), Error> {
    match (value, target) {
        (Value::Number(_), ColumnType::Number) => Ok(()),
        (Value::Varchar(s), ColumnType::Varchar(n)) => {
            if s.len() <= n {
                Ok(())
            } else {
                Err(Error::VarcharTooLong {
                    max: n,
                    got: s.len(),
                })
            }
        }
        (Value::Bool(_), ColumnType::Bool) => Ok(()),
        (Value::Bytes(b), ColumnType::Bytes(n)) => {
            if b.len() <= n {
                Ok(())
            } else {
                Err(Error::TypeMismatch {
                    expected: target.to_type(),
                    got: value.typ(),
                })
            }
        }
        _ => Err(Error::TypeMismatch {
            expected: target.to_type(),
            got: value.typ(),
        }),
    }
}

// Returns the type that an expression evaluates to
fn typecheck_expression(expr: &Expression, tables: &[&Table]) -> Result<Type, Error> {
    match expr {
        Expression::Literal(value) => Ok(value.typ()),
        Expression::Identifier(id) => {
            let col = resolve_column(id, tables)?;
            Ok(col.column_type.to_type())
        }
        Expression::BinaryOp { op, lhs, rhs } => {
            let lhs_type = typecheck_expression(lhs, tables)?;
            let rhs_type = typecheck_expression(rhs, tables)?;

            match op {
                Operator::Eq | Operator::Neq => {
                    let compatible = match (&lhs_type, &rhs_type) {
                        (Type::Varchar(s1), Type::Varchar(s2)) => s1 > s2,
                        (Type::Bytes(s1), Type::Bytes(s2)) => s1 > s2,
                        _ => lhs_type == rhs_type,
                    };
                    if !compatible {
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

/* CLAUSES*/

fn typecheck_join_clause(
    clause: &BoundJoinClause,
    tables: &[&Table], //@NOTE : This should include the clause.on_table as well, as it's a
                       //collection of ALL tables for the statement
) -> Result<(), Error> {
    if !((typecheck_expression(&clause.on, tables)?) == Type::Bool) {
        Err(Error::Parse("expression must be bool".into()))
    } else {
        Ok(())
    }
}

fn typecheck_where_clause(clause: &BoundWhereClause, tables: &[&Table]) -> Result<(), Error> {
    typecheck_expression(&clause.0, tables)?;
    Ok(())
}

fn typecheck_limit_clause(clause: &BoundLimitClause) -> Result<(), Error> {
    Ok(())
}
fn typecheck_orderby_clause(clause: &BoundOrderByClause) -> Result<(), Error> {
    /*
    is_orderable(clause.column)?; // Not all column types should be possible to order by
    * */
    Ok(())
}

/* STATEMENTS */
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
        check_value(v, c.column_type)?;
    }

    Ok(())
}
fn typecheck_select(stmt: &BoundSelectStatement) -> Result<(), Error> {
    // Collect all tables (base + joins)
    let mut tables: Vec<&Table> = vec![stmt.table];
    tables.extend(stmt.joins.iter().map(|j| j.on_table));

    stmt.joins
        .iter()
        .try_for_each(|clause| typecheck_join_clause(clause, &tables))?;

    stmt.where_clause
        .as_ref()
        .map(|clause| typecheck_where_clause(clause, &tables))
        .transpose()?;

    stmt.limit_clause
        .as_ref()
        .map(typecheck_limit_clause)
        .transpose()?;

    stmt.orderby_clause
        .as_ref()
        .map(typecheck_orderby_clause)
        .transpose()?;

    Ok(())
}

fn typecheck_delete(stmt: &BoundDeleteStatement) -> Result<(), Error> {
    let tables = [stmt.table];
    stmt.where_clause
        .as_ref()
        .map(|clause| typecheck_where_clause(clause, &tables))
        .transpose()?;

    Ok(())
}

/* DISPATCH */
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
    use crate::sql::ast::QualifiedIdentifier;
    use crate::storage::Table;
    use crate::storage::pager::Pager;
    use crate::types::ColumnType;
    use ordered_float::OrderedFloat;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn make_test_table() -> Table {
        let pager = Rc::new(RefCell::new(Pager::memory()));
        Table::new(
            "users".to_string(),
            [
                ("id".to_string(), ColumnType::Number),
                ("name".to_string(), ColumnType::Varchar(32)),
            ],
            pager,
        )
        .unwrap()
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
        assert!(matches!(
            result,
            Err(Error::VarcharTooLong { max: 32, got: 100 })
        ));
    }

    // Expression typechecking tests

    #[test]
    fn test_typecheck_expr_literal_number() {
        let table = make_test_table();
        let expr = Expression::Literal(Value::Number(OrderedFloat(42.0)));
        let result = typecheck_expression(&expr, &[&table]).unwrap();
        assert_eq!(result, Type::Number);
    }

    #[test]
    fn test_typecheck_expr_literal_string() {
        let table = make_test_table();
        let expr = Expression::Literal(Value::Varchar("hello".to_string()));
        let result = typecheck_expression(&expr, &[&table]).unwrap();
        assert!(matches!(result, Type::Varchar(_)));
    }

    #[test]
    fn test_typecheck_expr_identifier() {
        let table = make_test_table();
        let expr = Expression::Identifier(QualifiedIdentifier {
            qualifier: None,
            name: "id".to_string(),
        });
        let result = typecheck_expression(&expr, &[&table]).unwrap();
        assert_eq!(result, Type::Number);
    }

    #[test]
    fn test_typecheck_expr_unknown_column() {
        let table = make_test_table();
        let expr = Expression::Identifier(QualifiedIdentifier {
            qualifier: None,
            name: "nonexistent".to_string(),
        });
        let result = typecheck_expression(&expr, &[&table]);
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
        let result = typecheck_expression(&expr, &[&table]).unwrap();
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
        let result = typecheck_expression(&expr, &[&table]);
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
        let result = typecheck_expression(&expr, &[&table]).unwrap();
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
        let result = typecheck_expression(&expr, &[&table]);
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
        let result = typecheck_expression(&expr, &[&table]).unwrap();
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
        let result = typecheck_expression(&expr, &[&table]);
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
        let result = typecheck_expression(&expr, &[&table]).unwrap();
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
        let result = typecheck_expression(&expr, &[&table]);
        assert!(result.is_err());
    }

    #[test]
    fn test_typecheck_expr_column_comparison() {
        let table = make_test_table();
        // id == 42 (both numbers)
        let expr = Expression::BinaryOp {
            op: Operator::Eq,
            lhs: Box::new(Expression::Identifier(QualifiedIdentifier {
                qualifier: None,
                name: "id".to_string(),
            })),
            rhs: Box::new(Expression::Literal(Value::Number(OrderedFloat(42.0)))),
        };
        let result = typecheck_expression(&expr, &[&table]).unwrap();
        assert_eq!(result, Type::Bool);
    }
}
