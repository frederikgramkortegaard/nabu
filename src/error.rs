use crate::value::Type;

#[derive(Debug, Clone)]
pub enum Error {
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
    ColumnNotFound(String),

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
    DuplicateColumn(String),
    NoColumns,
    DuplicateTable(String),

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
}
