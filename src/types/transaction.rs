use crate::sql::ast::Statement;

#[derive(Debug)]
pub struct Transaction {
    pub statements: Vec<Statement>, // @NOTE : We don't store BoundStatement's as someone might've
    // dropped a Table or similar before this is executed, thus invalidating our statement.
    pub id: usize, //@NOTE : Not sure if this should be a usize, I guess it could be
}
