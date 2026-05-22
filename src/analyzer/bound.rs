use crate::sql::ast::{Expression, JoinKind, Value};
use crate::storage::Table;
use crate::types::Column;

/* CLAUSES */
#[derive(Debug)]
pub struct BoundWhereClause(pub Box<Expression>);

#[derive(Debug)]
pub struct BoundJoinClause<'a> {
    pub kind: JoinKind,
    pub on: Box<Expression>,
    pub on_table: &'a Table,
}

#[derive(Debug)]
pub struct BoundLimitClause {
    pub limit: usize,
    pub offset: usize,
}

#[derive(Debug)]
pub struct BoundOrderByClause<'a> {
    pub column: &'a Column,
}

/* STATEMENTS */

#[derive(Debug)]
pub struct BoundInsertStatement<'a> {
    pub values: Vec<Value>,
    pub table: &'a Table,
}

#[derive(Debug)]
pub struct BoundSelectStatement<'a> {
    pub columns: Vec<&'a Column>,
    pub table: &'a Table,
    pub joins: Vec<BoundJoinClause<'a>>,
    pub where_clause: Option<BoundWhereClause>,
    pub limit_clause: Option<BoundLimitClause>,
    pub orderby_clause: Option<BoundOrderByClause<'a>>,
}

#[derive(Debug)]
pub struct BoundDeleteStatement<'a> {
    pub table: &'a Table,
    pub where_clause: Option<BoundWhereClause>,
}

#[derive(Debug)]
pub enum BoundStatement<'a> {
    Insert(BoundInsertStatement<'a>),
    Select(BoundSelectStatement<'a>),
    Delete(BoundDeleteStatement<'a>),
}
