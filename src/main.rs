mod catalog;
mod constants;
mod error;
mod frontend;
mod provider;
mod query;
mod shared;
mod storage;

use arrow::datatypes::DataType;
use error::Error;
use frontend::lexer::LexerContext;
use frontend::parser::ParserContext;
use storage::{Database, TableBuilder};

fn run_query(db: &Database, sql: &str) -> Result<(), Error> {
    let tokens = LexerContext::lex(sql)?;
    let ast = ParserContext::parse(&tokens)?;
    let _resolved = query::resolve(ast, db)?;
    // TODO: execute resolved statement
    Ok(())
}

fn main() {
    env_logger::init();

    let mut db = Database::memory().unwrap();

    db.create_table(
        TableBuilder::new("users")
            .column("id", DataType::Float64, 8)
            .column("age", DataType::Float64, 8)
            .column("name", DataType::Utf8, 32),
    )
    .unwrap();

    let insert = run_query(&db, r#"INSERT (1, 25, "alice") INTO users"#);
    println!("Insert: {:?}", insert);

    let select = run_query(&db, "SELECT id, name FROM users WHERE age >= 18");
    println!("Select: {:?}", select);
}
