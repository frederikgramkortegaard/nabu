mod analyzer;
mod core;
mod error;
mod magic;
mod sql;
mod storage;
mod types;

use error::Error;
use sql::lexer::LexerContext;
use sql::parser::ParserContext;
use storage::{Database, TableBuilder};
use types::{ColumnType, QueryResult};

fn run_query(db: &Database, query: &str) -> Result<QueryResult, Error> {
    let tokens = LexerContext::lex(query)?;
    let ast = ParserContext::parse(&tokens)?;
    let bound = analyzer::bound::bind(ast, db)?;
    analyzer::typechecker::typecheck(&bound)?;
    let result = core::engine::execute(&bound)?;
    Ok(result)
}

fn main() {
    let mut mydb = Database::new("test.db").unwrap();

    mydb.create_table(
        TableBuilder::new("MyTable")
            .column("id", ColumnType::Number)
            .column("age", ColumnType::Number)
            .column("username", ColumnType::Varchar(32))
            .column("email", ColumnType::Varchar(256)),
    )
    .unwrap();
    /*
    for i in 0..100 {
        let result = run_query(
            &mydb,
            format!(
                "INSERT ({}, {}, \"alice\", \"alice@example.com\") INTO MyTable",
                i,
                rand::random_range(0..=100)
            )
            .as_str(),
        );
    }*/
    let result = run_query(&mydb, "SELECT _rowid, id, age, username FROM MyTable ");
    println!("{:?}", result);
}
