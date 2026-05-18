mod analyzer;
mod constants;
mod core;
mod error;
mod magic;
mod sql;
mod storage;
mod types;

use error::Error;
use rand::distr::{Alphanumeric, SampleString};
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

    let mut mydb = Database::new("test.db").unwrap();

    println!("Table exists: {}", mydb.table_exists("MyTable"));

    if !mydb.table_exists("MyTable") {
        println!("Creating table...");
        mydb.create_table(
            TableBuilder::new("MyTable")
                .column("id", ColumnType::Number)
                .column("age", ColumnType::Number)
                .column("username", ColumnType::Varchar(32))
                .column("email", ColumnType::Varchar(256)),
        )
        .unwrap();
        println!("Table created");

        println!("Inserting row...");
    }

    println!("Selecting...");
    let result = run_query(&mydb, "SELECT id, username FROM MyTable");
    println!("Select result: {:?}", result);
}
