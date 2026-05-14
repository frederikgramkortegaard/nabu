use super::ast::{Statement, Value};
use crate::storage::{ColumnType, Database};

#[derive(Debug, Clone)]
pub struct TypeError {
    pub message: String,
}

pub struct TypecheckerContext;

fn is_compatible_value(value: &Value, target: ColumnType) -> bool {
    match (value, target) {
        (Value::Number(_), ColumnType::Number) => true,
        (Value::Varchar(s), ColumnType::Varchar(n)) => s.len() <= n,
        _ => false,
    }
}

impl TypecheckerContext {
    pub fn typecheck(stmt: &Statement, db: &Database) -> Result<(), TypeError> {
        match stmt {
            Statement::Insert { values, table_name } => {
                let table = db.get_table(table_name).ok_or_else(|| TypeError {
                    message: format!("Table '{:?}' does not exist", table_name),
                })?;

                if values.len() != table.columns.len() {
                    return Err(TypeError {
                        message: "not enough values".to_string(),
                    });
                }

                for (v, c) in std::iter::zip(values, table.columns.values()) {
                    if !is_compatible_value(v, c.column_type) {
                        return Err(TypeError {
                            message: format!(
                                "Attempted to insert value of type '{:?}' into column '{:?}' of type '{:?}'",
                                v, c.name, c.column_type
                            ),
                        });
                    }
                }

                Ok(())
            }
            _ => Ok(()),
        }
    }
}
