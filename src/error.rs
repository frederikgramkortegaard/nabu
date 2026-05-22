use crate::types::Type;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),

    // Lexer
    UnterminatedString {
        row: usize,
        col: usize,
    },
    InvalidCharacter {
        ch: char,
        row: usize,
        col: usize,
    },

    // Parser
    UnexpectedToken {
        expected: String,
        got: String,
        row: usize,
        col: usize,
    },
    UnexpectedEof {
        expected: String,
    },
    Parse(String), // catch-all for parser errors

    // Binding
    TableNotFound(String),
    TableNotInScope(String),
    ColumnNotFound(String),
    ColumnNotFoundInTable { column: String, table: String },
    QualifierRequired { column: String },

    // Type checking
    TypeMismatch {
        expected: Type,
        got: Type,
    },
    WrongColumnCount {
        expected: usize,
        got: usize,
    },
    VarcharTooLong {
        max: usize,
        got: usize,
    },

    // Table
    ReservedColumnName(String),
    InvalidColumnName { name: String, reason: String },
    DuplicateColumn(String),
    NoColumns,
    DuplicateTable(String),
    TableNameTooLong(usize),
    ColumnsTooLong(usize),

    // Data access
    OutOfBounds {
        index: usize,
        len: usize,
    },
    CorruptedTree(String),
    WrongNodeType(String),
    NotInternalNode(String),

    // Engine
    ColumnNotInRow(String),
    DivisionByZero,

    IncorrectMagic(String),
}
impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}
