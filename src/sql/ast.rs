pub use crate::types::{Type, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClauseKind {
    Where,
    Join,
    OrderBy,
    Limit,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JoinKind {
    Inner,
    LeftOuter,
    RightOuter,
    FullOuter,
    Cross,
}

impl JoinKind {
    pub fn from_token(tag: &super::lexer::TokenType) -> Option<Self> {
        use super::lexer::TokenType;
        match tag {
            TokenType::Join | TokenType::InnerJoin => Some(JoinKind::Inner),
            TokenType::LeftOuterJoin => Some(JoinKind::LeftOuter),
            TokenType::RightOuterJoin => Some(JoinKind::RightOuter),
            TokenType::FullOuterJoin => Some(JoinKind::FullOuter),
            TokenType::CrossJoin => Some(JoinKind::Cross),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct WhereClause(pub Box<Expression>);

#[derive(Debug)]
pub struct JoinClause {
    pub kind: JoinKind,
    pub table: String,
    pub on: Box<Expression>,
}
#[derive(Debug)]
pub struct LimitClause {
    pub limit: usize,
    pub offset: usize,
}
#[derive(Debug, Clone)]
pub struct QualifiedIdentifier {
    pub qualifier: Option<String>,
    pub name: String,
}

#[derive(Debug)]
pub struct OrderByClause {
    pub column: QualifiedIdentifier,
}

#[derive(Debug)]
pub enum Operator {
    Eq,
    Neq,
    Leq,
    Geq,
    Lt,
    Gt,

    Add,
    Sub,
    Mul,
    Div,

    And,
    Or,
}

#[derive(Debug)]
pub enum Expression {
    Literal(Value),
    Identifier(QualifiedIdentifier),
    BinaryOp {
        op: Operator,
        lhs: Box<Expression>,
        rhs: Box<Expression>,
    },
}

#[derive(Debug)]
pub struct InsertStatement {
    pub values: Vec<Value>,
    pub table_name: String,
}

#[derive(Debug)]
pub struct SelectStatement {
    pub columns: Vec<String>,
    pub table: String,
    pub joins: Vec<JoinClause>,
    pub where_clause: Option<WhereClause>,
    pub limit_clause: Option<LimitClause>,
    pub orderby_clause: Option<OrderByClause>,
}

#[derive(Debug)]
pub struct DeleteStatement {
    pub table: String,
    pub where_clause: Option<WhereClause>,
}

#[derive(Debug)]
pub enum Statement {
    Insert(InsertStatement),
    Select(SelectStatement),
    Delete(DeleteStatement),
}
