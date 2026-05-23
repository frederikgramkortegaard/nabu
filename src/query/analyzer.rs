use super::Value;
use super::resolved::*;
use crate::catalog::Catalog;
use crate::error::Error;
use crate::frontend::ast::*;
use crate::shared::{DataType, FieldRef, SchemaExt, SchemaRef};

fn check_value(value: &Value, target: &DataType) -> Result<(), Error> {
    match (value, target) {
        (Value::Float64(_), DataType::Float64) => Ok(()),
        (Value::Utf8(_), DataType::Utf8) => Ok(()),
        (Value::Utf8(s), DataType::FixedSizeBinary(n)) => {
            if s.len() <= *n as usize {
                Ok(())
            } else {
                Err(Error::VarcharTooLong {
                    max: *n as usize,
                    got: s.len(),
                })
            }
        }
        (Value::Boolean(_), DataType::Boolean) => Ok(()),
        (Value::Binary(_), DataType::Binary) => Ok(()),
        (Value::Binary(b), DataType::FixedSizeBinary(n)) => {
            if b.len() <= *n as usize {
                Ok(())
            } else {
                Err(Error::TypeMismatch {
                    expected: target.clone(),
                    got: value.data_type(),
                })
            }
        }
        _ => Err(Error::TypeMismatch {
            expected: target.clone(),
            got: value.data_type(),
        }),
    }
}

pub fn resolve_join(join: Join, schemas: &[SchemaRef]) -> Result<ResolvedJoin, Error> {
    let schema = schemas
        .iter()
        .find(|s| s.table_name() == Some(&join.table))
        .cloned()
        .ok_or_else(|| Error::TableNotFound(join.table.clone()))?;

    let res_expr = resolve_expression(join.on, schemas)?;

    Ok(ResolvedJoin {
        kind: join.kind,
        schema,
        on: res_expr,
    })
}

pub fn resolve_expression(
    expr: Box<Expression>,
    schemas: &[SchemaRef],
) -> Result<ResolvedExpression, Error> {
    let (resolved, _type) = resolve_expression_typed(expr, schemas)?;
    Ok(resolved)
}

fn resolve_expression_typed(
    expr: Box<Expression>,
    schemas: &[SchemaRef],
) -> Result<(ResolvedExpression, DataType), Error> {
    match *expr {
        Expression::Literal(value) => {
            let typ = value.data_type();
            Ok((ResolvedExpression::Literal(value), typ))
        }
        Expression::Identifier(id) => {
            let field = resolve_column(&id, schemas)?;
            let typ = field.data_type().clone();
            Ok((ResolvedExpression::Column(field), typ))
        }
        Expression::BinaryOp { op, lhs, rhs } => {
            let (lhs_resolved, lhs_type) = resolve_expression_typed(lhs, schemas)?;
            let (rhs_resolved, rhs_type) = resolve_expression_typed(rhs, schemas)?;

            let result_type = match op {
                Operator::Eq | Operator::Neq => {
                    if lhs_type != rhs_type {
                        return Err(Error::TypeMismatch {
                            expected: lhs_type,
                            got: rhs_type,
                        });
                    }
                    DataType::Boolean
                }

                Operator::Lt | Operator::Gt | Operator::Leq | Operator::Geq => {
                    match (&lhs_type, &rhs_type) {
                        (DataType::Float64, DataType::Float64) => DataType::Boolean,
                        _ => {
                            return Err(Error::TypeMismatch {
                                expected: DataType::Float64,
                                got: lhs_type,
                            });
                        }
                    }
                }

                Operator::Add | Operator::Sub | Operator::Mul | Operator::Div => {
                    match (&lhs_type, &rhs_type) {
                        (DataType::Float64, DataType::Float64) => DataType::Float64,
                        _ => {
                            return Err(Error::TypeMismatch {
                                expected: DataType::Float64,
                                got: lhs_type,
                            });
                        }
                    }
                }

                Operator::And | Operator::Or => match (&lhs_type, &rhs_type) {
                    (DataType::Boolean, DataType::Boolean) => DataType::Boolean,
                    _ => {
                        return Err(Error::TypeMismatch {
                            expected: DataType::Boolean,
                            got: lhs_type,
                        });
                    }
                },
            };

            Ok((
                ResolvedExpression::BinaryOp {
                    op,
                    lhs: Box::new(lhs_resolved),
                    rhs: Box::new(rhs_resolved),
                },
                result_type,
            ))
        }
    }
}

/// Resolve a column identifier against available schemas.
/// Returns FieldRef (Arc<Field>) - cheap to clone.
pub fn resolve_column(
    id: &QualifiedIdentifier,
    schemas: &[SchemaRef],
) -> Result<FieldRef, Error> {
    let QualifiedIdentifier { qualifier, name } = id;

    match schemas.len() {
        0 => Err(Error::SchemaNotInScope("no schemas in scope".into())),
        1 => {
            // Single schema - just look up by name
            schemas[0]
                .field_with_name(name)
                .map(|f| f.clone().into())  // Field -> Arc<Field>
                .map_err(|_| Error::ColumnNotFound(name.clone()))
        }
        _ => {
            // Multiple schemas - require qualifier
            let qualifier = qualifier.as_ref().ok_or_else(|| Error::QualifierRequired {
                column: name.clone(),
            })?;

            let schema = schemas
                .iter()
                .find(|s| s.table_name() == Some(qualifier))
                .ok_or_else(|| Error::SchemaNotInScope(qualifier.clone()))?;

            schema
                .field_with_name(name)
                .map(|f| f.clone().into())
                .map_err(|_| Error::ColumnNotFoundInSchema {
                    column: name.clone(),
                    schema: qualifier.clone(),
                })
        }
    }
}

pub fn resolve(
    stmt: Statement,
    catalog: &impl Catalog,
) -> Result<ResolvedStatement, Error> {
    match stmt {
        Statement::Insert { table, values } => {
            let schema = catalog
                .get_schema(table.as_str())
                .ok_or(Error::TableNotFound(table))?;

            if values.len() != schema.fields().len() {
                return Err(Error::WrongColumnCount {
                    expected: schema.fields().len(),
                    got: values.len(),
                });
            }

            for (v, field) in std::iter::zip(&values, schema.fields()) {
                check_value(v, field.data_type())?;
            }

            Ok(ResolvedStatement::Insert { schema, values })
        }
        Statement::Select {
            table,
            columns,
            joins,
            filter,
            limit,
            offset,
            order_by,
        } => {
            let schema = catalog
                .get_schema(table.as_str())
                .ok_or(Error::TableNotFound(table))?;
            let all_schemas = catalog.get_schemas();

            // Resolve joins
            let res_joins: Vec<ResolvedJoin> = joins
                .into_iter()
                .map(|j| resolve_join(j, &all_schemas))
                .collect::<Result<Vec<_>, _>>()?;

            // Build list of schemas in scope (main table + joined tables)
            let mut available_schemas: Vec<SchemaRef> =
                res_joins.iter().map(|j| j.schema.clone()).collect();
            available_schemas.insert(0, schema.clone());

            // Resolve and validate output columns
            let res_columns: Vec<FieldRef> = columns
                .iter()
                .map(|col| resolve_column(col, &available_schemas))
                .collect::<Result<Vec<_>, _>>()?;

            let filter = filter
                .map(|f| resolve_expression(f, &available_schemas))
                .transpose()?;

            let order_by = order_by
                .map(|ob| resolve_column(&ob, &available_schemas))
                .transpose()?;

            Ok(ResolvedStatement::Select {
                schema,
                columns: res_columns,
                joins: res_joins,
                filter,
                limit,
                offset,
                order_by,
            })
        }
        Statement::Delete { table, filter } => {
            let schema = catalog
                .get_schema(table.as_str())
                .ok_or(Error::TableNotFound(table))?;

            let filter = filter
                .map(|f| resolve_expression(f, &[schema.clone()]))
                .transpose()?;

            Ok(ResolvedStatement::Delete { schema, filter })
        }
    }
}
