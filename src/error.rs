use crate::analyzer::bound::BindingError;
use crate::analyzer::typechecker::TypeError;
use crate::core::engine::EngineError;
use crate::sql::lexer::LexError;
use crate::sql::parser::ParseError;
use crate::storage::database::DatabaseError;
use crate::storage::table::TableError;

#[derive(Debug)]
pub enum Error {
    Lex(LexError),
    Parse(ParseError),
    Binding(BindingError),
    Type(TypeError),
    Engine(EngineError),
    Database(DatabaseError),
    Table(TableError),
}

impl From<LexError> for Error {
    fn from(e: LexError) -> Self {
        Error::Lex(e)
    }
}

impl From<ParseError> for Error {
    fn from(e: ParseError) -> Self {
        Error::Parse(e)
    }
}

impl From<BindingError> for Error {
    fn from(e: BindingError) -> Self {
        Error::Binding(e)
    }
}

impl From<TypeError> for Error {
    fn from(e: TypeError) -> Self {
        Error::Type(e)
    }
}

impl From<EngineError> for Error {
    fn from(e: EngineError) -> Self {
        Error::Engine(e)
    }
}

impl From<DatabaseError> for Error {
    fn from(e: DatabaseError) -> Self {
        Error::Database(e)
    }
}

impl From<TableError> for Error {
    fn from(e: TableError) -> Self {
        Error::Table(e)
    }
}
