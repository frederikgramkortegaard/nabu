mod analyzer;
mod column;
mod core;
mod cursor;
mod error;
mod node;
mod sql;
mod storage;
mod tree;
mod types;
mod value;

use error::Error;
use sql::lexer::LexerContext;
use sql::parser::ParserContext;
use storage::{ColumnType, Database, TableBuilder};
use types::QueryResult;

fn run_query(db: &Database, query: &str) -> Result<QueryResult, Error> {
    let tokens = LexerContext::lex(query)?;
    let ast = ParserContext::parse(&tokens)?;
    let bound = analyzer::bound::bind(ast, db)?;
    analyzer::typechecker::typecheck(&bound)?;
    let result = core::engine::execute(&bound)?;
    Ok(result)
}

fn main() {
    let mut mydb = Database::new();

    let table = TableBuilder::new("MyTable")
        .column("id", ColumnType::Number)
        .column("age", ColumnType::Number)
        .column("username", ColumnType::Varchar(32))
        .column("email", ColumnType::Varchar(256))
        .build()
        .unwrap();

    let _ = mydb.add_table(&table);

    let result = run_query(
        &mydb,
        "INSERT (1, 25, \"alice\", \"alice@example.com\") INTO MyTable",
    );
    println!("{:?}", result);
    let result = run_query(
        &mydb,
        "INSERT (1, 16, \"bob\", \"alice@example.com\") INTO MyTable",
    );
    println!("{:?}", result);
    let result = run_query(
        &mydb,
        "SELECT _rowid, id, age, username FROM MyTable where age >= 18",
    );
    println!("{:?}", result);

    let result = run_query(&mydb, "DELETE FROM MyTable where age >= 18");
    println!("{:?}", result);
    let result = run_query(
        &mydb,
        "SELECT _rowid, id, age, username FROM MyTable where age >= 18",
    );
    println!("{:?}", result);
}
