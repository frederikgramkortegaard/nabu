use super::plans::*;
use super::resolved::*;
use crate::error::Error;
use crate::shared::SchemaExt;

pub fn structure(stmt: ResolvedStatement) -> Result<LogicalPlan, Error> {
    let (table_name_opt, schema) = match &stmt {
        ResolvedStatement::Insert { schema, .. } => (schema.table_name(), schema),
        ResolvedStatement::Delete { schema, .. } => (schema.table_name(), schema),
        ResolvedStatement::Select { schema, .. } => (schema.table_name(), schema),
    };

    let table_name = table_name_opt.ok_or(Error::Parse(format!("")))?.to_string();

    let lplan = LogicalPlan::Scan {
        table_name: table_name.clone(),
        schema: schema.clone(),
        projection: None,
    };

    match stmt {
        ResolvedStatement::Select {
            columns,
            joins,
            filter,
            limit,
            offset,
            order_by,
            ..
        } => {
            let mut root = lplan;

            // Joins
            for join in joins.iter() {
                root = LogicalPlan::Join {
                    left: Box::new(root),
                    right: Box::new(LogicalPlan::Scan {
                        table_name: join
                            .schema
                            .table_name()
                            .ok_or(Error::Parse("".to_string()))?
                            .to_string(),
                        schema: join.schema.clone(),
                        projection: None,
                    }),

                    on: vec![(join.left_col.clone(), join.right_col.clone())],
                    join_type: join.kind,
                };
            }

            // Filter
            if let Some(f) = filter {
                root = LogicalPlan::Filter {
                    predicate: f,
                    input: Box::new(root),
                };
            }

            // Projection from original Select Statement
            // Since ResolvedStatement::Select for now sends in the Projection as a vector of
            // ColumnRef's and not expressions, we're going to wrap them in expressions for now,
            // until we start supporting more complex Expressions as projects
            root = LogicalPlan::Projection {
                expr: columns
                    .iter()
                    .map(|col| ResolvedExpression::Column(col.clone()))
                    .collect(),
                input: Box::new(root),
            };

            // Sorting
            // Again, we currently support sorting by a column, not anything complex
            // and also always descending
            if let Some(s) = order_by {
                root = LogicalPlan::Sort {
                    by: vec![(ResolvedExpression::Column(s.clone()), false)],
                    input: Box::new(root),
                };
            }

            // Limit & Offset
            if limit.is_some() || offset.is_some() {
                root = LogicalPlan::Limit {
                    limit,
                    offset,
                    input: Box::new(root),
                };
            }

            Ok(root)
        }

        ResolvedStatement::Delete { filter, .. } => {
            let mut root = lplan;

            if let Some(f) = filter {
                root = LogicalPlan::Filter {
                    predicate: f,
                    input: Box::new(root),
                };
            }

            Ok(root)
        }
        ResolvedStatement::Insert { schema, values } => Ok(LogicalPlan::Insert {
            table_name,
            schema: schema.clone(),
            values,
        }),
    }
}
