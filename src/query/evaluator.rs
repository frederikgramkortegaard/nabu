use arrow::datatypes::DataType;
use crate::error::Error;
use crate::frontend::ast::{Expression, Operator};
use super::Value;
use std::collections::HashMap;

pub fn eval_expr(expr: &Expression, row: &HashMap<&str, Value>) -> Result<Value, Error> {
    match expr {
        Expression::Literal(v) => Ok(v.clone()),

        // TODO: use qualifier to resolve column from correct table when joins are implemented
        Expression::Identifier(id) => row
            .get(id.name.as_str())
            .cloned()
            .ok_or_else(|| Error::ColumnNotInRow(id.name.clone())),

        Expression::BinaryOp { op, lhs, rhs } => {
            let l = eval_expr(lhs, row)?;
            let r = eval_expr(rhs, row)?;

            match op {
                // Equality
                Operator::Eq => Ok(Value::Boolean(l == r)),
                Operator::Neq => Ok(Value::Boolean(l != r)),

                // Comparison (numbers only)
                Operator::Lt => match (&l, &r) {
                    (Value::Float64(a), Value::Float64(b)) => Ok(Value::Boolean(a < b)),
                    _ => Err(Error::TypeMismatch {
                        expected: DataType::Float64,
                        got: l.data_type(),
                    }),
                },
                Operator::Gt => match (&l, &r) {
                    (Value::Float64(a), Value::Float64(b)) => Ok(Value::Boolean(a > b)),
                    _ => Err(Error::TypeMismatch {
                        expected: DataType::Float64,
                        got: l.data_type(),
                    }),
                },
                Operator::Leq => match (&l, &r) {
                    (Value::Float64(a), Value::Float64(b)) => Ok(Value::Boolean(a <= b)),
                    _ => Err(Error::TypeMismatch {
                        expected: DataType::Float64,
                        got: l.data_type(),
                    }),
                },
                Operator::Geq => match (&l, &r) {
                    (Value::Float64(a), Value::Float64(b)) => Ok(Value::Boolean(a >= b)),
                    _ => Err(Error::TypeMismatch {
                        expected: DataType::Float64,
                        got: l.data_type(),
                    }),
                },

                // Arithmetic
                Operator::Add => match (&l, &r) {
                    (Value::Float64(a), Value::Float64(b)) => Ok(Value::Float64(*a + *b)),
                    _ => Err(Error::TypeMismatch {
                        expected: DataType::Float64,
                        got: l.data_type(),
                    }),
                },
                Operator::Sub => match (&l, &r) {
                    (Value::Float64(a), Value::Float64(b)) => Ok(Value::Float64(*a - *b)),
                    _ => Err(Error::TypeMismatch {
                        expected: DataType::Float64,
                        got: l.data_type(),
                    }),
                },
                Operator::Mul => match (&l, &r) {
                    (Value::Float64(a), Value::Float64(b)) => Ok(Value::Float64(*a * *b)),
                    _ => Err(Error::TypeMismatch {
                        expected: DataType::Float64,
                        got: l.data_type(),
                    }),
                },
                Operator::Div => match (&l, &r) {
                    (Value::Float64(_), Value::Float64(b)) if *b == 0.0 => Err(Error::DivisionByZero),
                    (Value::Float64(a), Value::Float64(b)) => Ok(Value::Float64(*a / *b)),
                    _ => Err(Error::TypeMismatch {
                        expected: DataType::Float64,
                        got: l.data_type(),
                    }),
                },

                // Logical
                Operator::And => match (&l, &r) {
                    (Value::Boolean(a), Value::Boolean(b)) => Ok(Value::Boolean(*a && *b)),
                    _ => Err(Error::TypeMismatch {
                        expected: DataType::Boolean,
                        got: l.data_type(),
                    }),
                },
                Operator::Or => match (&l, &r) {
                    (Value::Boolean(a), Value::Boolean(b)) => Ok(Value::Boolean(*a || *b)),
                    _ => Err(Error::TypeMismatch {
                        expected: DataType::Boolean,
                        got: l.data_type(),
                    }),
                },
            }
        }
    }
}
