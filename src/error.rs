use arrow::datatypes::DataType;

#[derive(Debug)]
pub enum Error {
    // IO
    Io(std::io::Error),

    // Lexer
    UnterminatedString { row: usize, col: usize },
    InvalidCharacter { ch: char, row: usize, col: usize },

    // Parser
    UnexpectedToken { expected: String, got: String, row: usize, col: usize },
    UnexpectedEof { expected: String },
    Parse(String),

    // Binding
    TableNotFound(String),
    SchemaNotInScope(String),
    ColumnNotFound(String),
    ColumnNotFoundInSchema { column: String, schema: String },
    QualifierRequired { column: String },

    // Type checking
    TypeMismatch { expected: DataType, got: DataType },
    WrongColumnCount { expected: usize, got: usize },
    VarcharTooLong { max: usize, got: usize },

    // Table creation
    ReservedColumnName(String),
    InvalidColumnName { name: String, reason: String },
    DuplicateColumn(String),
    DuplicateTable(String),
    NoColumns,
    TableNameTooLong(usize),
    ColumnsTooLong(usize),

    // Storage
    OutOfBounds { index: usize, len: usize },
    CorruptedTree(String),
    WrongNodeType(String),
    IncorrectMagic(String),

    // Execution
    ColumnNotInRow(String),
    DivisionByZero,
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}
