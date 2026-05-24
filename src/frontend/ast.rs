pub use crate::shared::Value;

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

#[derive(Debug, Clone)]
pub struct QualifiedIdentifier {
    pub qualifier: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone, Copy)]
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
pub struct Join {
    pub kind: JoinKind,
    pub table: String,
    pub left_col: QualifiedIdentifier,
    pub right_col: QualifiedIdentifier,
}

#[derive(Debug)]
pub enum Statement {
    Insert {
        table: String,
        values: Vec<Value>,
    },
    Select {
        table: String,
        columns: Vec<QualifiedIdentifier>,
        joins: Vec<Join>,
        filter: Option<Box<Expression>>,
        limit: Option<usize>,
        offset: Option<usize>,
        order_by: Option<QualifiedIdentifier>,
    },
    Delete {
        table: String,
        filter: Option<Box<Expression>>,
    },
}
