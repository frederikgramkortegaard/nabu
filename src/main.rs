mod analyzer;
mod core;
mod error;
mod magic;
mod repl;
mod sql;
mod storage;
mod types;

use error::Error;
use rand::distr::{Alphanumeric, SampleString};
use repl::Repl;
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
    env_logger::init();

    let mut repl = Repl::new();
    let b = repl.start();
    println!("repl result: {:?}", b);

    return;

    let mut mydb = Database::new("test.db").unwrap();

    if !mydb.table_exists("MyTable") {
        mydb.create_table(
            TableBuilder::new("MyTable")
                .column("id", ColumnType::Number)
                .column("age", ColumnType::Number)
                .column("username", ColumnType::Varchar(32))
                .column("email", ColumnType::Varchar(256)),
        )
        .unwrap();

        for i in 0..10 {
            let result = run_query(
                &mydb,
                format!(
                    "INSERT ({}, {}, \"{a}\", \"{a}@example.com\") INTO MyTable",
                    i,
                    rand::random_range(0..=100),
                    a = Alphanumeric.sample_string(&mut rand::rng(), 16)
                )
                .as_str(),
            );
        }
    }
    let result = run_query(&mydb, "SELECT username, age FROM MyTable WHERE age >= 12");
    println!("{:?}", result);
}
