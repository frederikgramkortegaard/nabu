use crate::error::Error;
use crate::sql::ast::{Expression, Operator};
use crate::types::{Type, Value};
use std::collections::HashMap;
pub fn eval_expr(expr: &Expression, row: &HashMap<&str, Value>) -> Result<Value, Error> {
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
